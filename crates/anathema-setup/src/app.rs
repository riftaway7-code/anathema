//! The Windows setup GUI.

use crate::disk::{self, Disk};
use crate::install::{self, Mode};
use crate::logo::{self, EMBER, EMBER_DIM};
use anathema_setup::kernel_files::{self, KernelFiles};
use anathema_setup::stick::{same_stick, StickId};
use eframe::egui::{self, Color32, FontId, RichText, Stroke};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc, Mutex},
    thread,
};

const EFI: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/anathema.efi"));

const BG: Color32 = Color32::from_rgb(10, 10, 11);
const PANEL: Color32 = Color32::from_rgb(20, 20, 22);
const PANEL_HI: Color32 = Color32::from_rgb(30, 28, 28);
const TEXT: Color32 = Color32::from_rgb(232, 230, 227);
const DIM: Color32 = Color32::from_rgb(138, 135, 130);
const OK: Color32 = Color32::from_rgb(120, 200, 140);

#[derive(Default)]
pub struct Shared {
    log: Vec<String>,
    progress: f32,
    stage: String,
    result: Option<Result<(), String>>,
}

fn push_log(shared: &Arc<Mutex<Shared>>, line: String) {
    if let Ok(mut state) = shared.lock() {
        state.log.push(line);
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Page {
    Welcome,
    Drive,
    Review,
    Install,
    Done,
}

struct Entry {
    disk: Disk,
    prepared: Option<u64>,
    has_esp: bool,
}

struct App {
    page: Page,
    dry_run: bool,
    efi_override: Option<PathBuf>,
    entries: Vec<Entry>,
    scan_error: Option<String>,
    selected: Option<usize>,
    mode: Mode,
    confirmed: bool,
    erase_text: String,
    shared: Arc<Mutex<Shared>>,
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

impl App {
    fn new(dry_run: bool, efi_override: Option<PathBuf>) -> Self {
        let mut app = Self {
            page: Page::Welcome,
            dry_run,
            efi_override,
            entries: Vec::new(),
            scan_error: None,
            selected: None,
            mode: Mode::UpdateBoot,
            confirmed: false,
            erase_text: String::new(),
            shared: Arc::new(Mutex::new(Shared::default())),
        };
        app.scan();
        app
    }

    fn scan(&mut self) {
        match disk::list() {
            Ok(disks) => {
                self.entries = disks
                    .into_iter()
                    .map(|disk| Entry {
                        prepared: if disk.selectable { install::prepared_bytes(&disk) } else { None },
                        has_esp: install::esp_part(&disk).is_some(),
                        disk,
                    })
                    .collect();
                self.scan_error = None;
            }
            Err(error) => {
                self.entries.clear();
                self.scan_error = Some(error);
            }
        }
        self.selected = None;
        self.confirmed = false;
    }

    fn pick(&mut self, index: usize) {
        self.selected = Some(index);
        self.confirmed = false;
        self.erase_text.clear();
        let entry = &self.entries[index];
        self.mode = if entry.has_esp && entry.prepared.is_some() {
            Mode::UpdateBoot
        } else if entry.has_esp {
            Mode::UpdateBoot
        } else {
            Mode::Fresh
        };
    }

    fn ready(&self) -> bool {
        let Some(index) = self.selected else { return false };
        let entry = &self.entries[index];
        if !entry.disk.selectable {
            return false;
        }
        match self.mode {
            Mode::UpdateBoot => entry.has_esp && self.confirmed,
            Mode::Fresh => self.confirmed && self.erase_text.trim() == "ERASE",
        }
    }

    fn start_install(&mut self) {
        let Some(index) = self.selected else { return };
        let disk = self.entries[index].disk.clone();
        let mode = self.mode;
        let dry_run = self.dry_run;
        let efi_override = self.efi_override.clone();
        let shared = self.shared.clone();
        thread::spawn(move || {
            let outcome = run_install(&shared, &disk, mode, dry_run, efi_override);
            if let Ok(mut state) = shared.lock() {
                match &outcome {
                    Err(error) => state.log.push(format!("error: {error}")),
                    Ok(()) => {
                        state.progress = 1.0;
                        state.stage = "Finished".into();
                    }
                }
                state.result = Some(outcome);
            }
        });
    }
}

fn stick_id(d: &Disk) -> StickId {
    StickId { model: d.model.clone(), bytes: d.bytes, bus: d.bus, parts: d.parts.iter().map(|p| (p.offset, p.length, p.kind)).collect() }
}

fn download_note() -> String {
    format!(
        "Downloads about {:.1} GB of Apple kernel files from dortania's GitHub releases (once) and builds the kernel files on this PC",
        kernel_files::DOWNLOAD_BYTES as f64 / 1e9
    )
}

fn run_install(
    shared: &Arc<Mutex<Shared>>,
    disk: &Disk,
    mode: Mode,
    dry_run: bool,
    efi_override: Option<PathBuf>,
) -> Result<(), String> {
    let note = if dry_run { " (dry run, nothing is written)" } else { "" };
    push_log(shared, format!("Target disk {} {}{note}", disk.number, disk.model));
    if disk.number == 0 || !disk.selectable {
        return Err(format!("disk {} is not an allowed target: {}", disk.number, disk.reason));
    }
    let efi = match efi_override {
        Some(path) => std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?,
        None => EFI.to_vec(),
    };
    push_log(shared, format!("Boot image: {} bytes", efi.len()));
    let log_shared = shared.clone();
    let progress_shared = shared.clone();
    let mut log = move |line: String| push_log(&log_shared, line);
    let progress = move |value: f32, stage: &str| {
        if let Ok(mut state) = progress_shared.lock() {
            state.progress = value;
            state.stage = stage.to_owned();
        }
    };
    // the kernel files take the first 60% of the bar, writing the stick the rest
    let kernel = if dry_run {
        log(format!(
            "Would download about {:.1} GB of Kernel Debug Kits from dortania's GitHub releases and build the kernel files",
            kernel_files::DOWNLOAD_BYTES as f64 / 1e9
        ));
        KernelFiles { xkernel: Vec::new(), xkc: Vec::new() }
    } else {
        let cache = kernel_files::default_cache()?;
        std::fs::create_dir_all(&cache).map_err(|e| format!("cannot create {}: {e}", cache.display()))?;
        let free = disk::free_bytes(&cache.display().to_string());
        kernel_files::prepare(&cache, free, &mut |value, stage| progress(0.6 * value, stage), &mut log)?
    };
    // preparing can take many minutes: the stick must still be the one that was picked, and its
    // drive letters are read again
    let fresh;
    let disk = if dry_run {
        disk
    } else {
        fresh = disk::refresh_one(disk.number)?;
        same_stick(&stick_id(disk), &stick_id(&fresh))?;
        &fresh
    };
    let files = kernel_files::esp_files(&efi, &kernel);
    let mut install_progress = |value: f32, stage: &str| progress(0.6 + 0.4 * value, stage);
    let mut ctx = install::Context { dry_run, log: &mut log, progress: &mut install_progress };
    match mode {
        Mode::UpdateBoot => install::update_boot(disk, &efi, &files, &mut ctx),
        Mode::Fresh => install::fresh_install(disk, &files, &mut ctx),
    }
}

fn apply_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let v = &mut style.visuals;
    v.dark_mode = true;
    v.panel_fill = BG;
    v.window_fill = BG;
    v.extreme_bg_color = Color32::from_rgb(6, 6, 7);
    v.override_text_color = Some(TEXT);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(40, 38, 38));
    v.widgets.inactive.bg_fill = PANEL_HI;
    v.widgets.inactive.weak_bg_fill = PANEL_HI;
    v.widgets.hovered.bg_fill = Color32::from_rgb(60, 30, 20);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(60, 30, 20);
    v.widgets.active.bg_fill = EMBER_DIM;
    v.widgets.active.weak_bg_fill = EMBER_DIM;
    v.selection.bg_fill = EMBER;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(18.0, 8.0);
    ctx.set_style(style);
}

fn accent_button(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    let fill = if enabled { EMBER } else { Color32::from_rgb(50, 36, 32) };
    let color = if enabled { Color32::from_rgb(12, 8, 6) } else { DIM };
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).color(color).strong().size(15.0)).fill(fill),
    )
}

fn plain_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).size(15.0)))
}

fn heading(ui: &mut egui::Ui, title: &str, sub: &str) {
    ui.label(RichText::new(title).font(FontId::proportional(26.0)).color(TEXT).strong());
    ui.label(RichText::new(sub).color(DIM).size(14.0));
    ui.add_space(6.0);
}

fn card(ui: &mut egui::Ui, selected: bool, add: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let stroke = if selected {
        Stroke::new(1.5, EMBER)
    } else {
        Stroke::new(1.0, Color32::from_rgb(40, 38, 38))
    };
    let fill = if selected { PANEL_HI } else { PANEL };
    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(6.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        })
        .response
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.page == Page::Install {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        egui::TopBottomPanel::bottom("footer")
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(28, 14)))
            .show(ctx, |ui| self.footer(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(28, 22)))
            .show(ctx, |ui| match self.page {
                Page::Welcome => self.welcome(ui),
                Page::Drive => self.drive(ui),
                Page::Review => self.review(ui),
                Page::Install => self.install(ui),
                Page::Done => self.done(ui),
            });
    }
}

impl App {
    fn footer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mode = if self.dry_run { "dry run" } else { "live" };
            ui.label(RichText::new(format!("anathema setup  |  {mode}")).color(DIM).size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match self.page {
                Page::Welcome => {
                    if accent_button(ui, "Begin", true).clicked() {
                        self.page = Page::Drive;
                    }
                }
                Page::Drive => {
                    if accent_button(ui, "Next", self.selected.is_some()).clicked() {
                        self.page = Page::Review;
                    }
                    if plain_button(ui, "Back").clicked() {
                        self.page = Page::Welcome;
                    }
                    if plain_button(ui, "Rescan").clicked() {
                        self.scan();
                    }
                }
                Page::Review => {
                    if accent_button(ui, "Install", self.ready()).clicked() {
                        self.page = Page::Install;
                        self.start_install();
                    }
                    if plain_button(ui, "Back").clicked() {
                        self.page = Page::Drive;
                    }
                }
                Page::Install => {
                    let (finished, failed) = self
                        .shared
                        .lock()
                        .map(|s| (s.result.is_some(), matches!(s.result, Some(Err(_)))))
                        .unwrap_or((false, false));
                    if finished && !failed && accent_button(ui, "Continue", true).clicked() {
                        self.page = Page::Done;
                    }
                    if finished && failed && plain_button(ui, "Close").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                Page::Done => {
                    if accent_button(ui, "Close", true).clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            });
        });
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(14.0);
            logo::draw(ui, 150.0);
            ui.add_space(6.0);
            ui.label(RichText::new("ANATHEMA").size(34.0).color(EMBER).strong());
            ui.label(RichText::new("boot macOS on this pc").color(DIM).size(15.0));
            ui.add_space(18.0);
            ui.label(
                RichText::new(
                    "This setup puts the Anathema boot menu on a USB stick.\nA stick that already holds your macOS data only gets a new boot image, after a backup.",
                )
                .color(TEXT)
                .size(14.0),
            );
        });
    }

    fn drive(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "Choose the USB stick",
            "USB sticks and USB drives up to 256 GB can be selected. The Windows disk is locked. Unplug every USB drive you don't want erased.",
        );
        if let Some(error) = &self.scan_error {
            ui.colored_label(EMBER, format!("could not list disks: {error}"));
        }
        let mut pick = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, entry) in self.entries.iter().enumerate() {
                let disk = &entry.disk;
                let selected = self.selected == Some(i);
                let response = card(ui, selected, |ui| {
                    ui.horizontal(|ui| {
                        let name_color = if disk.selectable { TEXT } else { DIM };
                        ui.label(
                            RichText::new(format!("Disk {}  {}", disk.number, disk.model))
                                .color(name_color)
                                .size(15.0)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(gb(disk.bytes)).color(DIM));
                        });
                    });
                    let note = if disk.selectable {
                        match entry.prepared {
                            Some(bytes) => format!(
                                "{}  |  prepared data found: {}",
                                disk.reason,
                                gb(bytes)
                            ),
                            None => format!("{}  |  no prepared data", disk.reason),
                        }
                    } else {
                        format!("locked: {}", disk.reason)
                    };
                    let color = if disk.selectable && disk.removable { DIM } else { EMBER_DIM };
                    ui.label(RichText::new(note).color(color).size(13.0));
                });
                if disk.selectable && response.interact(egui::Sense::click()).clicked() {
                    pick = Some(i);
                }
            }
            if self.entries.is_empty() {
                ui.label(RichText::new("No disks found. Plug in the stick and press Rescan.").color(DIM));
            }
        });
        if let Some(i) = pick {
            self.pick(i);
        }
    }

    fn review(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.selected else { return };
        let (disk, prepared, has_esp) = {
            let e = &self.entries[index];
            (e.disk.clone(), e.prepared, e.has_esp)
        };
        heading(ui, "Review", "This is what the installer will do.");
        card(ui, false, |ui| {
            ui.label(
                RichText::new(format!("Target: disk {}  {}  ({})", disk.number, disk.model, gb(disk.bytes))).strong(),
            );
            if !disk.removable {
                ui.colored_label(
                    EMBER,
                    "This is a fixed USB drive, not a stick. External SSDs and backup drives look like this. Make sure it is the right disk.",
                );
            }
            // Show what is on the disk now, so a wrong pick is obvious before anything is erased.
            if disk.parts.is_empty() {
                ui.label(RichText::new("No partitions on this disk").color(DIM).size(13.0));
            } else {
                ui.label(RichText::new("On this disk now:").color(DIM).size(13.0));
                for p in &disk.parts {
                    let letter = p.letter.map(|l| format!("{l}:")).unwrap_or_else(|| "--".into());
                    let label = if p.label.is_empty() { "(no label)" } else { p.label.as_str() };
                    let fs = match (p.letter, p.fs.is_empty()) {
                        (None, _) => "no drive letter",
                        (Some(_), true) => "unformatted",
                        (Some(_), false) => p.fs.as_str(),
                    };
                    ui.label(RichText::new(format!("{letter}  {label}  {fs}  {}", gb(p.length))).size(13.0));
                }
            }
        });
        if has_esp {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, Mode::UpdateBoot, "Update boot only");
                ui.selectable_value(&mut self.mode, Mode::Fresh, "Erase and set up fresh");
            });
        }
        match self.mode {
            Mode::UpdateBoot => {
                card(ui, false, |ui| {
                    ui.label(RichText::new("Will be written").color(EMBER).strong());
                    ui.label("EFI partition: EFI/BOOT/BOOTX64.EFI and EFI/anathema/anathema.efi");
                    ui.label("Kernel files: anathema\\xkernel, xkc.bin and the xcp stubs");
                    ui.label(download_note());
                    ui.label(RichText::new(format!("Boot image {} bytes", EFI.len())).color(DIM).size(12.0));
                    let backup = install::backup_target(&disk)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(none)".into());
                    ui.label(format!("The current EFI folder is copied to {backup} first"));
                });
                card(ui, false, |ui| {
                    ui.label(RichText::new("Kept as it is").color(OK).strong());
                    match prepared {
                        Some(bytes) => ui.label(format!("{} of macOS data stays untouched", gb(bytes))),
                        None => ui.label("Nothing else on the stick is changed"),
                    };
                });
                ui.checkbox(&mut self.confirmed, "I checked the disk and want to update the boot image");
            }
            Mode::Fresh => {
                card(ui, false, |ui| {
                    ui.label(RichText::new("Will be written").color(EMBER).strong());
                    ui.label("New GPT: 200 MiB FAT32 EFI partition with EFI/BOOT/BOOTX64.EFI");
                    ui.label("Kernel files: anathema\\xkernel, xkc.bin and the xcp stubs");
                    ui.label(download_note());
                    ui.label("Data partition for macOS media (exFAT) on the rest");
                });
                card(ui, false, |ui| {
                    ui.label(RichText::new("Everything on this stick is erased").color(EMBER).strong());
                    if let Some(bytes) = prepared {
                        ui.colored_label(EMBER, format!("This includes {} of prepared macOS data.", gb(bytes)));
                    }
                });
                ui.checkbox(&mut self.confirmed, "I understand the whole disk is erased");
                ui.horizontal(|ui| {
                    ui.label("Type ERASE to continue:");
                    ui.text_edit_singleline(&mut self.erase_text);
                });
            }
        }
        if self.dry_run {
            ui.colored_label(EMBER, "Dry run: nothing will be written.");
        }
    }

    fn install(&mut self, ui: &mut egui::Ui) {
        let (stage, progress, log, result) = {
            let state = self.shared.lock().unwrap();
            (state.stage.clone(), state.progress, state.log.clone(), state.result.clone())
        };
        heading(ui, "Installing", &stage);
        ui.add(egui::ProgressBar::new(progress).fill(EMBER).desired_height(14.0).show_percentage());
        if let Some(Err(error)) = &result {
            ui.colored_label(EMBER, format!("Stopped: {error}"));
        }
        egui::Frame::new()
            .fill(Color32::from_rgb(6, 6, 7))
            .corner_radius(6.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(ui.available_height())
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for line in &log {
                            ui.label(RichText::new(line).monospace().size(12.0).color(Color32::from_rgb(200, 196, 190)));
                        }
                    });
            });
    }

    fn done(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            logo::draw(ui, 110.0);
            ui.label(RichText::new("Anathema is on the stick").size(26.0).strong());
            ui.add_space(10.0);
            ui.label("1. Safely eject the USB stick and restart the PC.");
            ui.label("2. Open the boot menu (usually F11, F12 or Esc during startup).");
            ui.label("3. Pick the USB stick. The Anathema menu appears.");
            if self.dry_run {
                ui.add_space(10.0);
                ui.colored_label(EMBER, "This was a dry run, nothing was written.");
            }
        });
    }
}

fn is_admin() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

fn relaunch_elevated() -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let exe: Vec<u16> = std::env::current_exe()
        .ok()
        .map(|p| p.into_os_string().encode_wide().chain([0]).collect())
        .unwrap_or_default();
    let args: Vec<u16> = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ")
        .encode_utf16()
        .chain([0])
        .collect();
    let verb: Vec<u16> = "runas".encode_utf16().chain([0]).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            exe.as_ptr(),
            args.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    result as usize > 32
}

pub fn main() -> eframe::Result {
    let mut dry_run = false;
    let mut efi_override = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dry-run" | "-n" => dry_run = true,
            "--allow-virtual" => disk::ALLOW_VIRTUAL.store(true, Ordering::Relaxed),
            "--efi" => efi_override = args.next().map(PathBuf::from),
            _ => {}
        }
    }
    if let Some(at) = std::env::args().position(|a| a == "--cli") {
        let rest: Vec<String> = std::env::args().skip(at + 1).collect();
        let mode = if rest.first().map(String::as_str) == Some("fresh") { Mode::Fresh } else { Mode::UpdateBoot };
        let number: u32 = rest.get(1).and_then(|n| n.parse().ok()).unwrap_or(0);
        let log_path = rest.get(2).cloned().unwrap_or_else(|| "anathema-cli.log".into());
        let shared = Arc::new(Mutex::new(Shared::default()));
        let outcome = match disk::refresh_one(number) {
            Ok(d) => run_install(&shared, &d, mode, dry_run, efi_override),
            Err(e) => Err(e),
        };
        let state = shared.lock().unwrap();
        let mut text = state.log.join("
");
        text.push_str(&format!("
result: {outcome:?}
"));
        let _ = std::fs::write(log_path, text);
        return Ok(());
    }
    if !dry_run && !is_admin() && relaunch_elevated() {
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 600.0])
            .with_min_inner_size([680.0, 520.0])
            .with_title("Anathema Setup"),
        ..Default::default()
    };
    eframe::run_native(
        "Anathema Setup",
        options,
        Box::new(move |cc| {
            apply_style(&cc.egui_ctx);
            Ok(Box::new(App::new(dry_run, efi_override)))
        }),
    )
}

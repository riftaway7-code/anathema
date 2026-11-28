use alloc::{string::{String, ToString}, vec::Vec};
pub struct Prop {
    pub name: String,
    pub value: Vec<u8>,
}

pub struct Node {
    pub props: Vec<Prop>,
    pub children: Vec<Node>,
}

fn u32le(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

fn parse_at(d: &[u8], at: &mut usize) -> Node {
    let nprops = u32le(d, *at) as usize;
    let nchildren = u32le(d, *at + 4) as usize;
    *at += 8;
    let mut props = Vec::new();
    for _ in 0..nprops {
        let raw = &d[*at..*at + 32];
        let end = raw.iter().position(|&b| b == 0).unwrap_or(32);
        let name = String::from_utf8_lossy(&raw[..end]).into_owned();
        let len = (u32le(d, *at + 32) & 0x7fff_ffff) as usize;
        let value = d[*at + 36..*at + 36 + len].to_vec();
        *at += 36 + ((len + 3) & !3);
        props.push(Prop { name, value });
    }
    let mut children = Vec::new();
    for _ in 0..nchildren {
        children.push(parse_at(d, at));
    }
    Node { props, children }
}

pub fn parse(d: &[u8]) -> Node {
    let mut at = 0;
    parse_at(d, &mut at)
}

impl Node {
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.props.iter().find(|p| p.name == key).map(|p| p.value.as_slice())
    }

    pub fn name(&self) -> String {
        self.get("name").map(|v| String::from_utf8_lossy(v).trim_end_matches('\0').to_string()).unwrap_or_default()
    }

    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name() == name)
    }
}

pub fn show(v: &[u8]) -> String {
    if v.is_empty() {
        return String::from("(empty)");
    }
    let text = v.iter().all(|&b| b == 0 || (0x20..0x7f).contains(&b)) && v.iter().any(|&b| b != 0);
    if text {
        return format!("\"{}\"", String::from_utf8_lossy(v).trim_end_matches('\0'));
    }
    if v.len() <= 64 {
        let mut out = String::new();
        if v.len() % 8 == 0 {
            for c in v.chunks(8) {
                out.push_str(&format!("{:#x} ", u64::from_le_bytes(c.try_into().unwrap())));
            }
        } else if v.len() % 4 == 0 {
            for c in v.chunks(4) {
                out.push_str(&format!("{:#x} ", u32::from_le_bytes(c.try_into().unwrap())));
            }
        } else {
            for b in v {
                out.push_str(&format!("{:02x}", b));
            }
        }
        return out.trim_end().to_string();
    }
    format!("<{} bytes>", v.len())
}

pub fn dump(n: &Node, depth: usize, max_depth: usize, skip: &[&str]) {
    let pad = "  ".repeat(depth);
    println!("{}{}", pad, n.name());
    for p in &n.props {
        if p.name == "name" || p.name.starts_with("Unused") || p.name == "AAPL,phandle" || skip.contains(&p.name.as_str()) {
            continue;
        }
        println!("{}  {} = {}", pad, p.name, show(&p.value));
    }
    if depth < max_depth {
        for c in &n.children {
            dump(c, depth + 1, max_depth, skip);
        }
    }
}

impl Node {
    pub fn set(&mut self, key: &str, value: Vec<u8>) {
        if let Some(p) = self.props.iter_mut().find(|p| p.name == key) {
            p.value = value;
        } else {
            self.props.push(Prop { name: key.to_string(), value });
        }
    }

    pub fn child_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.children.iter_mut().find(|c| c.name() == name)
    }
}

fn put(n: &Node, out: &mut Vec<u8>) {
    out.extend_from_slice(&(n.props.len() as u32).to_le_bytes());
    out.extend_from_slice(&(n.children.len() as u32).to_le_bytes());
    for p in &n.props {
        let mut name = [0u8; 32];
        let b = p.name.as_bytes();
        name[..b.len().min(31)].copy_from_slice(&b[..b.len().min(31)]);
        out.extend_from_slice(&name);
        out.extend_from_slice(&(p.value.len() as u32).to_le_bytes());
        out.extend_from_slice(&p.value);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    for c in &n.children {
        put(c, out);
    }
}

pub fn serialize(n: &Node) -> Vec<u8> {
    let mut out = Vec::new();
    put(n, &mut out);
    out
}

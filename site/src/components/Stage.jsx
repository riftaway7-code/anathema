import { useEffect, useRef, useState } from "react";
import { useMotionValueEvent, useReducedMotion } from "motion/react";
import { stage } from "../content.js";

// the hero product shot: frames rendered in blender (cycles) by site/render/monitor.py.
// on load the screen powers on (off, then the firmware handing off to anathema-xboot, then
// the kernel); scrolling then turns the monitor to face the visitor by drawing the matching
// frame on a canvas. the markup is the same on the server and the client: the first frame is
// a plain <img> (largest paint, works without js), and css keeps the screen dark until the
// intro runs (see the inline script in index.html). reduced motion: no intro, front view.

const frameSrc = (dir, i) => `${dir}/turn_${String(i).padStart(3, "0")}.webp`;

export default function Stage({ progress }) {
  const still = !!useReducedMotion();
  const canvas = useRef(null);
  const frames = useRef([]);
  const shown = useRef(-1);
  const [intro, setIntro] = useState("pending");
  const [scrubbing, setScrubbing] = useState(false);

  function draw(i, force = false) {
    const el = canvas.current;
    const img = frames.current[i];
    if (!el || !img || !img.complete || !img.naturalWidth) return false;
    if (i === shown.current && !force) return true;
    if (el.width !== img.naturalWidth || el.height !== img.naturalHeight) {
      el.width = img.naturalWidth;
      el.height = img.naturalHeight;
    }
    const ctx = el.getContext("2d");
    ctx.clearRect(0, 0, el.width, el.height);
    ctx.drawImage(img, 0, 0);
    shown.current = i;
    return true;
  }

  // load the frame set that suits this screen; reduced motion needs only the front view
  useEffect(() => {
    const el = canvas.current;
    const dir = el.clientWidth * (window.devicePixelRatio || 1) > 1000 ? stage.large : stage.small;
    const wanted = still ? [stage.frames - 1] : [...Array(stage.frames).keys()];
    const list = [];
    for (const i of wanted) {
      const img = new Image();
      img.decoding = "async";
      img.src = frameSrc(dir, i);
      list[i] = img;
    }
    frames.current = list;
    if (still) {
      list[stage.frames - 1].onload = () => {
        draw(stage.frames - 1, true);
        setScrubbing(true);
      };
    }
  }, [still]);

  // power-on sequence
  useEffect(() => {
    if (still) {
      setIntro("done");
      return;
    }
    setIntro("off");
    const a = setTimeout(() => setIntro("loader"), 450);
    const b = setTimeout(() => setIntro("done"), 2000);
    return () => {
      clearTimeout(a);
      clearTimeout(b);
    };
  }, [still]);

  useMotionValueEvent(progress, "change", (v) => {
    if (still) return;
    const i = Math.min(stage.frames - 1, Math.max(0, Math.round(v * (stage.frames - 1))));
    for (let d = 0; d <= i; d++) {
      if (draw(i - d)) break; // the nearest frame at or before i that has loaded
    }
    if (i > 0 && !scrubbing) {
      setIntro("done");
      setScrubbing(true);
    }
  });

  return (
    <figure className="stage">
      <div className={`stage__box intro-${intro}${scrubbing ? " is-scrubbing" : ""}`}>
        <img
          className="stage__frame stage__frame--base"
          src={`${stage.large}/turn_000.webp`}
          width={stage.width}
          height={stage.height}
          alt={stage.alt}
          fetchPriority="high"
        />
        <img className="stage__frame stage__frame--loader" src={stage.bootLoader} width={stage.width} height={stage.height} alt="" aria-hidden="true" />
        <img className="stage__frame stage__frame--off" src={stage.bootOff} width={stage.width} height={stage.height} alt="" aria-hidden="true" />
        <canvas ref={canvas} className="stage__frame stage__canvas" aria-hidden="true" />
      </div>
      <figcaption>
        <span className="tok" translate="no">
          Darwin 27.2.0 · xnu-13432.41.4 · RELEASE_X86_64
        </span>
        <span>{stage.caption}</span>
      </figcaption>
    </figure>
  );
}

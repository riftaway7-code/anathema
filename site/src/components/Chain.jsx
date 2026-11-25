import { useRef } from "react";
import { m, useReducedMotion, useScroll, useTransform } from "motion/react";
import useMedia from "../useMedia.js";
import { how, screens } from "../content.js";

// on wide screens the section pins: scrolling walks through the boot chain one step at a
// time, and the display beside it switches from the loader's screen to the kernel's at the
// step where the kernel takes over. phones and reduced motion get a plain list instead.

function Step({ progress, i, n, step }) {
  const a = i / n;
  const b = (i + 1) / n;
  const range = i === 0 ? [0, b - 0.02, b + 0.02] : i === n - 1 ? [a - 0.02, a + 0.02, 1] : [a - 0.02, a + 0.02, b - 0.02, b + 0.02];
  const values = i === 0 ? [1, 1, 0.5] : i === n - 1 ? [0.5, 1, 1] : [0.5, 1, 1, 0.5];
  const opacity = useTransform(progress, range, values);
  return (
    <m.li className={step.ours ? "step step--ours" : "step"} style={{ opacity }}>
      <p className="step__name">{step.name}</p>
      <p className="step__text">{step.text}</p>
    </m.li>
  );
}

function Pinned() {
  const ref = useRef(null);
  const n = how.chain.length;
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start start", "end end"] });
  // the kernel's screen takes over where the chain reaches the kernel
  const handoff = how.chain.findIndex((s) => s.screen === "kernel") / n;
  const loaderOpacity = useTransform(scrollYProgress, [handoff - 0.03, handoff + 0.01], [1, 0]);
  const rail = useTransform(scrollYProgress, [0, 1], [0, 1]);
  return (
    <section id="how" className="chain chain--pinned" ref={ref} style={{ height: `${n * 45 + 25}vh` }} aria-labelledby="how-title">
      <div className="chain__sticky">
        <div className="wrap chain__grid">
          <div>
            <h2 id="how-title">{how.title}</h2>
            <div className="chain__track">
              <m.span className="chain__rail" style={{ scaleY: rail }} aria-hidden="true" />
              <ol className="chain__steps" role="list">
                {how.chain.map((s, i) => (
                  <Step key={s.name} progress={scrollYProgress} i={i} n={n} step={s} />
                ))}
              </ol>
            </div>
          </div>
          <div className="chain__screen">
            <img src={screens.kernel.src} width={1280} height={720} alt={screens.kernel.alt} loading="lazy" decoding="async" />
            <m.img
              src={screens.loader.src}
              width={1280}
              height={720}
              alt={screens.loader.alt}
              loading="lazy"
              decoding="async"
              style={{ opacity: loaderOpacity }}
            />
          </div>
        </div>
      </div>
    </section>
  );
}

function Plain() {
  return (
    <section id="how" className="chain wrap" aria-labelledby="how-title">
      <h2 id="how-title">{how.title}</h2>
      <ol className="chain__steps chain__steps--plain" role="list">
        {how.chain.map((s) => (
          <li key={s.name} className={s.ours ? "step step--ours" : "step"}>
            <p className="step__name">{s.name}</p>
            <p className="step__text">{s.text}</p>
          </li>
        ))}
      </ol>
      <div className="chain__pair">
        <img className="chain__shot pan" src={screens.loader.src} width={1280} height={720} alt={screens.loader.alt} loading="lazy" decoding="async" />
        <img className="chain__shot pan" src={screens.kernel.src} width={1280} height={720} alt={screens.kernel.alt} loading="lazy" decoding="async" />
      </div>
    </section>
  );
}

export default function Chain() {
  const reduce = useReducedMotion();
  const wide = useMedia("(min-width: 960px)");
  return (
    <>
      {wide && !reduce ? <Pinned /> : <Plain />}
      <p className="wrap chain__note">{how.note}</p>
    </>
  );
}

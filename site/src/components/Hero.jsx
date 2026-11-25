import { useRef } from "react";
import { useScroll } from "motion/react";
import { ArrowUpRightIcon, DiscordLogoIcon } from "@phosphor-icons/react";
import Stage from "./Stage.jsx";
import { hero, cta } from "../content.js";

export default function Hero() {
  const ref = useRef(null);
  // 0 at the top of the page, 1 once the hero's end reaches the bottom of the viewport:
  // by then the monitor faces the visitor, whole and centred on screen
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start start", "end end"] });
  return (
    <section className="hero" ref={ref} aria-labelledby="hero-title">
      <div className="wrap hero__copy">
        <p className="hero__eyebrow">{hero.eyebrow}</p>
        <h1 id="hero-title">{hero.title}</h1>
        <p className="hero__sub">{hero.sub}</p>
        <div className="hero__ctas">
          <a className="btn btn--primary btn--large" href={cta.discord.href}>
            <DiscordLogoIcon size={20} weight="fill" aria-hidden="true" />
            {cta.discord.label}
          </a>
          <a className="btn btn--ghost btn--large" href={cta.github.href}>
            {cta.github.label}
            <ArrowUpRightIcon size={18} aria-hidden="true" />
          </a>
        </div>
      </div>
      <div className="wrap wrap--wide">
        <Stage progress={scrollYProgress} />
      </div>
    </section>
  );
}

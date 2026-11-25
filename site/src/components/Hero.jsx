import { m, useReducedMotion } from "motion/react";
import { DiscordLogoIcon, GithubLogoIcon } from "@phosphor-icons/react";
import { hero } from "../content.js";

const rise = (reduce, delay) => ({
  initial: reduce ? false : { opacity: 0, y: 20 },
  animate: { opacity: 1, y: 0 },
  transition: { duration: 0.7, delay, ease: [0.16, 1, 0.3, 1] },
});

export default function Hero() {
  const reduce = useReducedMotion();
  return (
    <section className="hero wrap" aria-labelledby="hero-title">
      <div className="hero__copy">
        <m.h1 id="hero-title" {...rise(reduce, 0)}>
          {hero.title}
        </m.h1>
        <m.p className="hero__sub" {...rise(reduce, 0.08)}>
          {hero.sub}
        </m.p>
        <m.div className="hero__ctas" {...rise(reduce, 0.16)}>
          <a className="btn btn--primary" href={hero.primary.href}>
            <DiscordLogoIcon size={20} weight="fill" aria-hidden="true" />
            {hero.primary.label}
          </a>
          <a className="btn btn--ghost" href={hero.secondary.href}>
            <GithubLogoIcon size={20} aria-hidden="true" />
            {hero.secondary.label}
          </a>
        </m.div>
      </div>
      <m.figure className="hero__shot" {...rise(reduce, 0.24)}>
        <div className="screen">
          <img
            src={hero.image.src}
            width={hero.image.width}
            height={hero.image.height}
            alt={hero.image.alt}
            fetchPriority="high"
          />
        </div>
        <figcaption>{hero.image.caption}</figcaption>
      </m.figure>
    </section>
  );
}

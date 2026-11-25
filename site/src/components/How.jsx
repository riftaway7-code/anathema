import Reveal from "./Reveal.jsx";
import { how } from "../content.js";

// the boot chain reads left to right on wide screens and top to bottom on phones.
// each link appears in order, which is the story the section tells.
export default function How() {
  return (
    <section id="how" className="section section--band" aria-labelledby="how-title">
      <div className="wrap">
        <Reveal>
          <h2 id="how-title">{how.title}</h2>
        </Reveal>
        <ol className="chain">
          {how.chain.map((c, i) => (
            <Reveal as="li" key={c.name} delay={i * 0.08} className={i === 1 ? "link link--ours" : "link"}>
              <p className="link__name">{c.name}</p>
              <p className="link__text">{c.text}</p>
            </Reveal>
          ))}
        </ol>
        <Reveal as="p" className="lede chain__note">
          {how.note}
        </Reveal>
      </div>
    </section>
  );
}

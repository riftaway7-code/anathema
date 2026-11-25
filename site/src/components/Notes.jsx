import { PlusIcon } from "@phosphor-icons/react";
import Reveal from "./Reveal.jsx";
import { notes, FAQ } from "../content.js";

// native <details>: keyboard and screen reader behaviour for free, no js state
export default function Notes() {
  return (
    <section className="section wrap notes" aria-labelledby="notes-title">
      <Reveal className="notes__head">
        <h2 id="notes-title">{notes.title}</h2>
        <p className="lede">
          More in the <a href={FAQ}>FAQ</a>.
        </p>
      </Reveal>
      <Reveal className="notes__list" delay={0.08}>
        {notes.items.map((n) => (
          <details key={n.q} className="note">
            <summary>
              {n.q}
              <PlusIcon className="note__mark" size={20} aria-hidden="true" />
            </summary>
            <p>{n.a}</p>
          </details>
        ))}
      </Reveal>
    </section>
  );
}

import { PlusIcon } from "@phosphor-icons/react";
import { notes } from "../content.js";

// native <details>: keyboard and screen reader behaviour for free, no js state
export default function Notes() {
  return (
    <section id="faq" className="section wrap notes" aria-labelledby="notes-title">
      <div className="notes__head reveal">
        <h2 id="notes-title">{notes.title}</h2>
        <p className="lede">
          {notes.lede.text} <a href={notes.lede.link.href}>{notes.lede.link.label}</a>.
        </p>
      </div>
      <div className="notes__list">
        {notes.items.map((n) => (
          <details key={n.q} className="note">
            <summary>
              {n.q}
              <PlusIcon className="note__mark" size={20} weight="light" aria-hidden="true" />
            </summary>
            <p>{n.a}</p>
          </details>
        ))}
      </div>
    </section>
  );
}

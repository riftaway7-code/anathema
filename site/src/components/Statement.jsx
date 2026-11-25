import { statement } from "../content.js";

// four short lines, each brightening as it scrolls into the middle of the viewport (css
// scroll-driven animation, see .statement__line). without support, or with reduced motion,
// they simply show.
export default function Statement() {
  return (
    <section className="statement wrap" aria-label="What Anathema runs">
      <p className="statement__text">
        {statement.map((t) => (
          <span key={t} className="statement__line">
            {t}
          </span>
        ))}
      </p>
    </section>
  );
}

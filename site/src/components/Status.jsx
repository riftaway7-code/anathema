import { status } from "../content.js";

// a compatibility table, the format this audience trusts for "what works"
export default function Status() {
  const { labels } = status;
  return (
    <section id="status" className="section wrap" aria-labelledby="status-title">
      <div className="reveal">
        <h2 id="status-title">{status.title}</h2>
        <p className="meta">{status.updated}</p>
        <p className="lede">{status.sub}</p>
      </div>
      <ul className="ledger" role="list">
        {status.rows.map((r) => (
          <li key={r.name} className="ledger__row reveal">
            <span className={`tag tag--${r.state}`}>{labels[r.state]}</span>
            <p className="ledger__name">{r.name}</p>
            <p className="ledger__text">{r.text}</p>
          </li>
        ))}
        <li className="ledger__row reveal">
          <span className="tag tag--none">{labels.none}</span>
          <ul className="chips ledger__chips" role="list">
            {status.none.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </li>
      </ul>
    </section>
  );
}

import { footer } from "../content.js";

export default function Footer() {
  return (
    <footer className="footer">
      <div className="wrap footer__inner">
        <div>
          <a className="wordmark" href="/">
            <img src="/favicon.svg" alt="" width="24" height="24" />
            Anathema
          </a>
          <p className="footer__line">{footer.line}</p>
        </div>
        <nav className="footer__links" aria-label="Footer">
          {footer.links.map((l) => (
            <a key={l.label} href={l.href}>
              {l.label}
            </a>
          ))}
          <a href={footer.studio.href}>{footer.studio.label}</a>
        </nav>
      </div>
    </footer>
  );
}

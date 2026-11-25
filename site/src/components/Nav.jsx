import { GithubLogoIcon } from "@phosphor-icons/react";
import { nav, REPO, hero } from "../content.js";

export default function Nav() {
  return (
    <header className="nav">
      <div className="nav__inner wrap">
        <a className="wordmark" href="#main" aria-label="Anathema home">
          <img src="/favicon.svg" alt="" width="24" height="24" />
          Anathema
        </a>
        <nav className="nav__links" aria-label="Main">
          {nav.map((item) => (
            <a key={item.label} href={item.href}>
              {item.label}
            </a>
          ))}
        </nav>
        <div className="nav__actions">
          <a className="icon-link" href={REPO} aria-label="Anathema on GitHub">
            <GithubLogoIcon size={22} weight="regular" />
          </a>
          <a className="btn btn--primary btn--small" href={hero.primary.href}>
            {hero.primary.label}
          </a>
        </div>
      </div>
    </header>
  );
}

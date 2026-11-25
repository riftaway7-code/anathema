import { ArrowUpRightIcon, DiscordLogoIcon } from "@phosphor-icons/react";
import { help, cta } from "../content.js";

export default function Help() {
  return (
    <section id="help" className="help" aria-labelledby="help-title">
      <div className="wrap">
        <div className="help__main reveal">
          <h2 id="help-title">{help.title}</h2>
          <p className="help__sub">
            {help.sub} Post in{" "}
            <code className="tok" translate="no">
              {help.channel}
            </code>
            .
          </p>
          <a className="btn btn--primary btn--large" href={cta.discord.href}>
            <DiscordLogoIcon size={20} weight="fill" aria-hidden="true" />
            {cta.discord.label}
          </a>
        </div>
        <div className="help__more">
          {help.items.map((h) => (
            <div key={h.name} className="help__item reveal">
              <h3>{h.name}</h3>
              <p>
                {h.text}
                {h.channel && (
                  <>
                    {" "}
                    Use{" "}
                    <code className="tok" translate="no">
                      {h.channel}
                    </code>
                    .
                  </>
                )}
              </p>
              <a className="text-link" href={h.link.href}>
                {h.link.label}
                <ArrowUpRightIcon size={16} aria-hidden="true" />
              </a>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

import { DesktopTowerIcon, ListChecksIcon, BugIcon, ArrowUpRightIcon, DiscordLogoIcon } from "@phosphor-icons/react";
import Reveal from "./Reveal.jsx";
import { help, hero } from "../content.js";

export default function Help() {
  return (
    <section className="section wrap" aria-labelledby="help-title">
      <Reveal>
        <h2 id="help-title">{help.title}</h2>
      </Reveal>
      <div className="bento">
        <Reveal className="tile tile--test">
          <DesktopTowerIcon size={32} aria-hidden="true" />
          <h3>{help.test.name}</h3>
          <p>{help.test.text}</p>
          <a className="btn btn--dark" href={hero.primary.href}>
            <DiscordLogoIcon size={20} weight="fill" aria-hidden="true" />
            {hero.primary.label}
          </a>
        </Reveal>
        <Reveal className="tile tile--task" delay={0.08}>
          <ListChecksIcon size={28} aria-hidden="true" />
          <h3>{help.task.name}</h3>
          <p>{help.task.text}</p>
          <a className="text-link" href={help.task.link.href}>
            {help.task.link.label}
            <ArrowUpRightIcon size={16} aria-hidden="true" />
          </a>
        </Reveal>
        <Reveal className="tile tile--report" delay={0.16}>
          <BugIcon size={28} aria-hidden="true" />
          <h3>{help.report.name}</h3>
          <p>{help.report.text}</p>
        </Reveal>
      </div>
    </section>
  );
}

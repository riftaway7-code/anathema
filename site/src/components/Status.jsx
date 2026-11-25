import {
  PowerIcon,
  StackIcon,
  TerminalWindowIcon,
  LifebuoyIcon,
  CpuIcon,
  UsbIcon,
} from "@phosphor-icons/react";
import Reveal from "./Reveal.jsx";
import { status } from "../content.js";

const icons = {
  boot: PowerIcon,
  stack: StackIcon,
  terminal: TerminalWindowIcon,
  lifebuoy: LifebuoyIcon,
  cpu: CpuIcon,
  usb: UsbIcon,
};

export default function Status() {
  return (
    <section id="status" className="section wrap" aria-labelledby="status-title">
      <Reveal>
        <h2 id="status-title">{status.title}</h2>
        <p className="lede">{status.sub}</p>
      </Reveal>
      <div className="status">
        <div className="status__working">
          <h3 className="status__label">Working</h3>
          <ul className="works">
            {status.working.map((w, i) => {
              const Icon = icons[w.icon];
              return (
                <Reveal as="li" key={w.name} delay={i * 0.05} className="work">
                  <Icon className="work__icon" size={24} aria-hidden="true" />
                  <div>
                    <p className="work__name">{w.name}</p>
                    <p className="work__text">{w.text}</p>
                  </div>
                </Reveal>
              );
            })}
          </ul>
        </div>
        <Reveal className="status__side" delay={0.1}>
          <h3 className="status__label">In progress</h3>
          <ul className="progress">
            {status.progress.map((p) => (
              <li key={p.name}>
                <p className="work__name">{p.name}</p>
                <p className="work__text">{p.text}</p>
              </li>
            ))}
          </ul>
          <h3 className="status__label">Not yet</h3>
          <ul className="chips" aria-label="Not working yet">
            {status.notYet.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </Reveal>
      </div>
    </section>
  );
}

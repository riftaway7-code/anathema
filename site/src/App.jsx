import { LazyMotion, domAnimation } from "motion/react";
import Nav from "./components/Nav.jsx";
import Hero from "./components/Hero.jsx";
import Statement from "./components/Statement.jsx";
import Chain from "./components/Chain.jsx";
import Status from "./components/Status.jsx";
import Notes from "./components/Notes.jsx";
import Help from "./components/Help.jsx";
import Footer from "./components/Footer.jsx";

export default function App() {
  // motion is only used for scroll-linked values (the hero turn, the pinned boot chain)
  return (
    <LazyMotion features={domAnimation} strict>
      <a className="skip" href="#main">
        Skip to content
      </a>
      <Nav />
      <main id="main">
        <Hero />
        <Statement />
        <Chain />
        <Status />
        <Notes />
        <Help />
      </main>
      <Footer />
    </LazyMotion>
  );
}

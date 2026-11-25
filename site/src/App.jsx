import { LazyMotion, domAnimation } from "motion/react";
import Nav from "./components/Nav.jsx";
import Hero from "./components/Hero.jsx";
import Status from "./components/Status.jsx";
import How from "./components/How.jsx";
import Help from "./components/Help.jsx";
import Notes from "./components/Notes.jsx";
import Footer from "./components/Footer.jsx";

export default function App() {
  // only the dom animation features: the page uses fades and in-view reveals, nothing else
  return (
    <LazyMotion features={domAnimation} strict>
      <a className="skip" href="#main">
        Skip to content
      </a>
      <Nav />
      <main id="main">
        <Hero />
        <Status />
        <How />
        <Help />
        <Notes />
      </main>
      <Footer />
    </LazyMotion>
  );
}

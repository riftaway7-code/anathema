import { m, useReducedMotion } from "motion/react";

// fades a block in as it enters the viewport, once. this is the only scroll motion on the
// page: it sets the reading order of each section. reduced motion shows everything at once.
export default function Reveal({ as = "div", delay = 0, children, ...rest }) {
  const reduce = useReducedMotion();
  const Tag = m[as];
  return (
    <Tag
      initial={reduce ? false : { opacity: 0, y: 18 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, amount: 0.25 }}
      transition={{ duration: 0.6, delay, ease: [0.16, 1, 0.3, 1] }}
      {...rest}
    >
      {children}
    </Tag>
  );
}

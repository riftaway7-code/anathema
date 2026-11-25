import { useEffect, useState } from "react";

// true while the media query matches. starts false so the server render and the first client
// render agree, then updates after mount and on changes (resize, rotation), not on scroll.
export default function useMedia(query) {
  const [match, setMatch] = useState(false);
  useEffect(() => {
    const mq = window.matchMedia(query);
    const on = () => setMatch(mq.matches);
    on();
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, [query]);
  return match;
}

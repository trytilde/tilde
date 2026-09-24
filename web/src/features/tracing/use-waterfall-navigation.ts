import { useEffect, useLayoutEffect, useRef, useState } from "react";

/** Timeline navigation: zoom changes time scale, while vertical position is native scrolling. */
export function useWaterfallNavigation(ready: boolean, maxScale: number, onNavigate: () => void) {
  const frame = useRef<HTMLDivElement>(null);
  const scroll = useRef<HTMLDivElement>(null);
  const dragged = useRef(false);
  const pointerStart = useRef({ x: 0, y: 0, left: 0, top: 0 });
  const [camera, setCamera] = useState({ scale: 1, positionX: 0, positionY: 0 });
  const current = useRef(camera);
  const pendingLeft = useRef<number | null>(null);
  const limit = useRef(maxScale);
  limit.current = maxScale;
  const notify = useRef(onNavigate);
  notify.current = onNavigate;
  function onScroll() {
    const element = scroll.current;
    if (!element) return;
    current.current = {
      ...current.current,
      positionX: -element.scrollLeft,
      positionY: -element.scrollTop,
    };
    setCamera(current.current);
    notify.current();
  }
  function zoomToPoint(next: number, clientX?: number) {
    const element = scroll.current;
    if (!element) return;
    const previous = current.current.scale;
    const scale = Math.max(1, Math.min(limit.current, next));
    if (scale === previous) return;
    const x =
      clientX === undefined
        ? element.clientWidth / 2
        : clientX - element.getBoundingClientRect().left;
    const left = pendingLeft.current ?? element.scrollLeft;
    pendingLeft.current = scale === 1 ? 0 : Math.max(0, ((left + x) * scale) / previous - x);
    current.current = { ...current.current, scale, positionX: -pendingLeft.current };
    setCamera(current.current);
    notify.current();
  }
  // Apply the horizontal anchor after React has rendered the new timeline width.
  useLayoutEffect(() => {
    if (!scroll.current || pendingLeft.current === null) return;
    scroll.current.scrollLeft = pendingLeft.current;
    pendingLeft.current = null;
    onScroll();
  }, [camera.scale]);
  useEffect(() => {
    const element = scroll.current;
    if (!element) return;
    let startScale = 1;
    let active = false;
    type Gesture = Event & { scale: number; clientX: number };
    const start = (event: Event) => {
      event.preventDefault();
      active = true;
      dragged.current = true;
      startScale = current.current.scale;
      notify.current();
    };
    const change = (event: Event) => {
      if (!active) return;
      event.preventDefault();
      const gesture = event as Gesture;
      zoomToPoint(startScale * gesture.scale, gesture.clientX);
    };
    const end = (event: Event) => {
      event.preventDefault();
      active = false;
    };
    const wheel = (event: WheelEvent) => {
      // Trackpad pinch is ctrl+wheel in Chromium/Firefox. Regular wheel stays native pan.
      if (!event.ctrlKey) return;
      event.preventDefault();
      if (active) return; // Safari can emit both gesture and wheel events.
      const delta =
        event.deltaY *
        (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? element.clientHeight : 1);
      zoomToPoint(current.current.scale * Math.exp(-delta * 0.005), event.clientX);
    };
    element.addEventListener("gesturestart", start, { passive: false });
    element.addEventListener("gesturechange", change, { passive: false });
    element.addEventListener("gestureend", end, { passive: false });
    element.addEventListener("wheel", wheel, { passive: false });
    return () => {
      element.removeEventListener("gesturestart", start);
      element.removeEventListener("gesturechange", change);
      element.removeEventListener("gestureend", end);
      element.removeEventListener("wheel", wheel);
    };
  }, [ready]);
  return {
    frame,
    scroll,
    camera,
    dragged,
    pointerStart,
    onScroll,
    zoomIn: () => zoomToPoint(current.current.scale * 1.5),
    zoomOut: () => zoomToPoint(current.current.scale / 1.5),
    fit: () => zoomToPoint(1),
  };
}

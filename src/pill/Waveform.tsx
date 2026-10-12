import { useEffect, useRef, type RefObject } from "react";
import { smoothLevel } from "./envelope";

const SPACING = 4.5;
const SAMPLE_MS = 75;
const HEIGHT = 22;
// Pale sky blue at the top falling to soft teal-blue, like `pill-wave` in globals.css.
const WAVE_TOP = "#d2edff";
const WAVE_BOTTOM = "#6cc3f0";

/** Draw at the display's frame rate; microphone events only update a target. */
export function Waveform({ level }: { level: RefObject<{ value: number; updatedAt: number }> }) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const element = canvas.current;
    const context = element?.getContext("2d");
    if (!element || !context) return;
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const history = new Array<number>(128).fill(0);
    let width = 0;
    let scale = 1;
    let previous = performance.now();
    let sampleTime = 0;
    let smoothed = 0;
    let frame = 0;
    let fill: CanvasGradient | string = WAVE_TOP;

    const resize = () => {
      width = element.getBoundingClientRect().width;
      scale = Math.min(window.devicePixelRatio || 1, 2);
      element.width = Math.round(width * scale);
      element.height = HEIGHT * scale;
      context.setTransform(scale, 0, 0, scale, 0, 0);
      fill = context.createLinearGradient(0, 0, 0, HEIGHT);
      fill.addColorStop(0, WAVE_TOP);
      fill.addColorStop(1, WAVE_BOTTOM);
    };
    const observer = new ResizeObserver(resize);
    observer.observe(element);
    resize();

    const draw = (now: number) => {
      const elapsed = Math.min(now - previous, 50);
      previous = now;
      // A disconnected microphone must settle back to silence.
      const target = now - level.current.updatedAt < 250 ? level.current.value : 0;
      smoothed = smoothLevel(smoothed, target, elapsed);
      sampleTime += elapsed;
      while (sampleTime >= SAMPLE_MS) {
        history.pop();
        history.unshift(smoothed);
        sampleTime -= SAMPLE_MS;
      }

      context.clearRect(0, 0, width, HEIGHT);
      context.fillStyle = fill;
      const count = Math.ceil(width / SPACING) + 1;
      for (let i = 0; i < count; i++) {
        // Older sound drifts away to the left and fades, like a trail of water.
        context.globalAlpha = 1 - 0.55 * (i / count);
        // Continuous horizontal travel replaces per-event jumps and restarted
        // CSS transitions. Adjacent samples soften isolated peaks.
        const amplitude = reducedMotion.matches
          ? smoothed * (0.55 + 0.45 * Math.sin((i / count) * Math.PI))
          : history[i] * 0.6 + (history[i + 1] ?? 0) * 0.4;
        const height = 3 + amplitude * (HEIGHT - 3);
        const x = width - 2.5 - (i + (reducedMotion.matches ? 0 : sampleTime / SAMPLE_MS)) * SPACING;
        context.beginPath();
        context.roundRect(x, (HEIGHT - height) / 2, 2.5, height, 1.25);
        context.fill();
      }
      context.globalAlpha = 1;
      frame = requestAnimationFrame(draw);
    };
    frame = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [level]);

  return <canvas ref={canvas} aria-hidden="true" className="h-[22px] min-w-0 flex-1" />;
}

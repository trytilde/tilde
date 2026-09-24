import { useEffect, useRef, useState } from "react";
import { GrainGradient, PaperTexture } from "@paper-design/shaders-react";

// Same deep beige field as tilde-marketing's ResultCardShader; paper comes from PageEffects.
const colors = ["#9d8b72", "#b09e84", "#c0ae96", "#cebea8"];
export default function SidebarMaterial() {
  const layer = useRef<HTMLDivElement>(null);
  const [supported, setSupported] = useState(false);
  const [visible, setVisible] = useState(false);
  const [reducedMotion, setReducedMotion] = useState(true);
  const [pageVisible, setPageVisible] = useState(true);
  useEffect(() => {
    // Keep the CSS material when WebGL is disabled or would fall back to software rendering.
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl2", { failIfMajorPerformanceCaveat: true });
    if (gl) {
      setSupported(true);
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    }
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const updateMotion = () => setReducedMotion(motion.matches);
    const updateVisibility = () => setPageVisible(!document.hidden);
    updateMotion();
    updateVisibility();
    motion.addEventListener("change", updateMotion);
    document.addEventListener("visibilitychange", updateVisibility);
    const observer = new IntersectionObserver(([entry]) => setVisible(entry.isIntersecting));
    if (layer.current) observer.observe(layer.current);
    return () => {
      observer.disconnect();
      motion.removeEventListener("change", updateMotion);
      document.removeEventListener("visibilitychange", updateVisibility);
    };
  }, []);
  return (
    <div ref={layer} className="tilde-sidebar-material" aria-hidden="true">
      {supported && (
        <div className="tilde-sidebar-grain">
          <GrainGradient
            width="100%"
            height="100%"
            colorBack="#ddd5c7"
            colors={colors}
            frame={0}
            intensity={0.5}
            noise={0.25}
            offsetX={0}
            offsetY={0}
            rotation={0}
            shape="corners"
            softness={0.5}
            speed={visible && pageVisible && !reducedMotion ? 1 : 0}
            minPixelRatio={1}
            maxPixelCount={200_000}
          />
        </div>
      )}
      <div className="tilde-sidebar-glass" />
      {supported && (
        <div className="tilde-sidebar-paper">
          <PaperTexture
            width="100%"
            height="100%"
            colorBack="#ddd5c7"
            colorFront="#9d8b72"
            roughness={0.25}
            speed={0}
            minPixelRatio={1}
            maxPixelCount={200_000}
          />
        </div>
      )}
    </div>
  );
}

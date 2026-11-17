// docs/layers.feature LA3a: captions whose words light up as they're said, over full-frame footage
import { AbsoluteFill, Composition, Video, registerRoot, staticFile, useCurrentFrame } from "raea";

const WORDS = ["Each", "word", "lights", "up"];

function Edit() {
  const f = useCurrentFrame();
  const lit = Math.min(WORDS.length - 1, Math.floor(f / 12));
  return (
    <AbsoluteFill style={{ background: "#000" }}>
      <Video src={staticFile("clip.mp4")} style={{ position: "absolute", inset: 0, width: "100%", height: "100%", objectFit: "cover" }} />
      <div data-raea-role="caption" style={{ position: "absolute", left: 0, right: 0, bottom: 80, textAlign: "center", fontSize: 60, fontWeight: 800, color: "#fff" }}>
        {WORDS.map((w, i) => (
          <span key={w} style={{ color: i === lit ? "#ffd400" : "#fff" }}>{w} </span>
        ))}
      </div>
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="edit" width={1280} height={720} fps={30} durationInFrames={60} component={Edit} />);

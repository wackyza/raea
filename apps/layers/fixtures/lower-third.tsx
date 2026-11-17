// docs/layers.feature LA3a: full-frame footage with a lower third (a bar and words) and captions
import { AbsoluteFill, Composition, Sequence, Video, registerRoot, staticFile } from "raea";

function Edit() {
  return (
    <AbsoluteFill style={{ background: "#000" }}>
      <Video src={staticFile("clip.mp4")} style={{ position: "absolute", inset: 0, width: "100%", height: "100%", objectFit: "cover" }} />
      <div style={{ position: "absolute", left: 0, bottom: 200, width: 620, height: 90, background: "#ff3b30" }}>
        <span style={{ position: "absolute", left: 32, top: 18, fontSize: 44, color: "#fff" }}>Jane Doe, raea</span>
      </div>
      <Sequence from={10} durationInFrames={40}>
        <div data-raea-role="caption" style={{ position: "absolute", left: 0, right: 0, bottom: 60, textAlign: "center", fontSize: 52, color: "#fff", textShadow: "0 2px 6px #000" }}>
          The words as they're said
        </div>
      </Sequence>
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="edit" width={1280} height={720} fps={30} durationInFrames={60} component={Edit} />);

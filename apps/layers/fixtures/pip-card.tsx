// docs/layers.feature LA3a: footage as a picture in picture on a coloured card, over a background
import { AbsoluteFill, Composition, Video, registerRoot, staticFile } from "raea";

function Edit() {
  return (
    <AbsoluteFill style={{ background: "#14161c" }}>
      <div style={{ position: "absolute", left: 120, top: 120, width: 700, height: 420, background: "#2050ff", borderRadius: 24 }}>
        <Video src={staticFile("clip.mp4")} style={{ position: "absolute", left: 30, top: 30, width: 640, height: 360 }} />
      </div>
      <div style={{ position: "absolute", left: 880, top: 160, fontSize: 56, fontWeight: 800, color: "#fff" }}>On the card</div>
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="edit" width={1280} height={720} fps={30} durationInFrames={60} component={Edit} />);

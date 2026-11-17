// docs/layers.feature LA3a: 3D with a title over it and a caption (the 3D is in the graphics layer, never the captions')
import { AbsoluteFill, Composition, registerRoot, useCurrentFrame } from "raea";
import { Entity } from "raea/3d";
import { StandardMaterial } from "raea/pbr";
import { Cuboid, Mesh3d } from "raea/mesh";
import { AmbientLight, DirectionalLight } from "raea/light";
import { Quat, Transform } from "raea/transform";
import { Camera3d } from "raea/camera";

function Edit() {
  const turn = (useCurrentFrame() / 30) * (Math.PI / 3);
  return (
    <AbsoluteFill style={{ background: "#14161c" }}>
      <Camera3d transform={Transform.fromXyz(0, 1.6, 6).lookingAt([0, 0.5, 0])} projection={{ Perspective: { fov: (40 * Math.PI) / 180 } }} />
      <AmbientLight brightness={160} />
      <DirectionalLight transform={Transform.fromXyz(3, 5, 4).lookingAt([0, 0, 0])} illuminance={11200} />
      <Entity transform={Transform.IDENTITY.withRotation(Quat.fromEuler("XYZ", 0, turn, 0))}>
        <Mesh3d transform={Transform.fromXyz(0, 0.6, 0)}>
          <Cuboid half_size={[0.6, 0.6, 0.6]} />
          <StandardMaterial base_color="#e8b04a" perceptual_roughness={0.35} />
        </Mesh3d>
      </Entity>
      <div style={{ position: "absolute", top: 60, width: "100%", textAlign: "center", fontSize: 64, fontWeight: 800, color: "#fff" }}>Made to be seen</div>
      <div data-raea-role="caption" style={{ position: "absolute", bottom: 60, width: "100%", textAlign: "center", fontSize: 48, color: "#ff0" }}>a caption over 3D</div>
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="edit" width={1280} height={720} fps={30} durationInFrames={60} component={Edit} />);

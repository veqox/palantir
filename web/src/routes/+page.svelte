<script lang="ts">
    import { Globe } from "$lib/geometries/globe";
    import { Trace } from "$lib/geometries/trace";
    import type { Event, Peer } from "$lib/types/event";
    import { formatBytes, relativeTimeFormatter } from "$lib/utils/format";
    import { toCartesian } from "$lib/utils/geo";
    import { Camera, Color, Orbit, Polyline, Quat, Raycast, Renderer, Transform, Vec3, type OGLRenderingContext } from "ogl";
    import { onMount } from "svelte";
    import { SvelteDate } from "svelte/reactivity";

    let canvas: HTMLCanvasElement;

    const now: SvelteDate = new SvelteDate();

    $effect(() => {
        const interval = setInterval(() => {
            now.setTime(Date.now());
        }, 500);

        return () => {
            clearInterval(interval);
        };
    });

    let peers: Peer[] = $state([]);
    let sortedPeers: Peer[] = $state([]);

    $effect(() => {
        sortedPeers = peers
            .filter((p) => now.getTime() - p.last_message.getTime() < 1000 * 60)
            .sort((a, b) => b.ingress_bytes + b.egress_bytes - (a.ingress_bytes + a.egress_bytes));
    });

    let camera: Camera;
    let controls: Orbit;
    let scene: Transform;
    let gl: OGLRenderingContext;

    onMount(() => {
        const renderer = new Renderer({ dpr: 2, webgl: 2, canvas });
        gl = renderer.gl;
        camera = new Camera(gl, { fov: 40 });
        camera.position.set(0, 0, 4);
        controls = new Orbit(camera, { target: new Vec3() });

        function resize() {
            renderer.setSize(window.innerWidth, window.innerHeight);
            camera.perspective({ aspect: gl.canvas.width / gl.canvas.height });
        }
        window.addEventListener("resize", resize, false);
        resize();

        scene = new Transform();

        const globe = new Globe(gl);
        globe.setParent(scene);

        const source = new EventSource("http://localhost:3000/events");

        source.onmessage = async (e) => {
            const data = JSON.parse(e.data, (_, value) => {
                if (value && typeof value == "object" && "secs_since_epoch" in value && "nanos_since_epoch" in value) {
                    return new Date(value.secs_since_epoch * 1000 + value.nanos_since_epoch / 1_000_000);
                }
                return value;
            }) as Event;

            if (data.peer) {
                const { peer } = data;

                peers.push(peer);
            }

            if (data.packet) {
                const { src_addr, dst_addr, bytes, timestamp, proto } = data.packet;

                const src_peer = peers.find((p) => p.addr === src_addr);
                const dst_peer = peers.find((p) => p.addr === dst_addr);

                if (!src_peer || !dst_peer) {
                    return;
                }

                src_peer.ingress_bytes += bytes;
                dst_peer.egress_bytes += bytes;

                src_peer.last_message = timestamp;
                dst_peer.last_message = timestamp;

                const from = toCartesian({ lat: src_peer.info.lat, lon: src_peer.info.lon });
                const to = toCartesian({ lat: dst_peer.info.lat, lon: dst_peer.info.lon });
                const color = proto === "Tcp" ? new Color("#00FF00") : proto === "Udp" ? new Color("#FF0000") : new Color("#0000FF");

                const trace = new Trace(gl, { from, to, color });
                trace.setParent(scene);

                await trace.fadeIn(1000);
                await trace.fadeOut(1000);
                scene.removeChild(trace);
            }
        };

        function update() {
            renderer.render({ scene, camera });
            controls.update();
            requestAnimationFrame(update);
        }
        requestAnimationFrame(update);
    });
</script>

<canvas bind:this={canvas} class="block h-screen w-screen"></canvas>

<div class="absolute right-8 top-8 text-white p-4 border backdrop-blur-md">
    <table class="border-separate border-spacing-x-2">
        <tbody>
            {#each sortedPeers as peer}
                <tr>
                    <td>{peer.addr}</td>
                    <td class="text-green-500">{formatBytes(peer.ingress_bytes)}</td>
                    <td class="text-red-500">{formatBytes(peer.egress_bytes)}</td>
                    <td>{relativeTimeFormatter.format(Math.round((peer.last_message.getTime() - now.getTime()) / 1000), "second")}</td>
                    <td>
                        <button title="location" type="button" onclick={() => {}} class="cursor-pointer">
                            <svg xmlns="http://www.w3.org/2000/svg" height="24px" viewBox="0 -960 960 960" width="24px" class="fill-white">
                                <path
                                    d="M480-80q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q152 0 263.5 98T876-538q-20-10-41.5-15.5T790-560q-19-73-68.5-130T600-776v16q0 33-23.5 56.5T520-680h-80v80q0 17-11.5 28.5T400-560h-80v80h240q11 0 20.5 5.5T595-459q-17 27-26 57t-9 62q0 63 32.5 117T659-122q-41 20-86 31t-93 11Zm-40-82v-78q-33 0-56.5-23.5T360-320v-40L168-552q-3 18-5.5 36t-2.5 36q0 121 79.5 212T440-162Zm340 82q-7 0-12-4t-7-10q-11-35-31-65t-43-59q-21-26-34-57t-13-65q0-58 41-99t99-41q58 0 99 41t41 99q0 34-13.5 64.5T873-218q-23 29-43 59t-31 65q-2 6-7 10t-12 4Zm0-113q10-17 22-31.5t23-29.5q14-19 24.5-40.5T860-340q0-33-23.5-56.5T780-420q-33 0-56.5 23.5T700-340q0 24 10.5 45.5T735-254q12 15 23.5 29.5T780-193Zm0-97q-21 0-35.5-14.5T730-340q0-21 14.5-35.5T780-390q21 0 35.5 14.5T830-340q0 21-14.5 35.5T780-290Z"
                                />
                            </svg>
                        </button>
                    </td>
                </tr>
            {/each}
        </tbody>
    </table>
</div>

<script lang="ts">
    import { Globe } from "$lib/geometries/globe";
    import { Trace } from "$lib/geometries/trace";
    import type { Event, Peer } from "$lib/types/event";
    import { formatBytes, formatFlag } from "$lib/utils/format";
    import { toCartesian } from "$lib/utils/geo";
    import { Camera, Orbit, Quat, Renderer, Transform, Vec3, type OGLRenderingContext } from "ogl";
    import { onMount } from "svelte";

    let canvas: HTMLCanvasElement;

    let peers: Peer[] = $state([]);

    let orbit: Orbit;
    let camera: Camera;

    onMount(() => {
        const renderer = new Renderer({ dpr: 2, webgl: 2, canvas });
        const gl = renderer.gl;
        camera = new Camera(gl, { fov: 40 });
        camera.position.set(0, 0, 4);
        orbit = new Orbit(camera, { target: new Vec3() });

        function resize() {
            renderer.setSize(window.innerWidth, window.innerHeight);
            camera.perspective({ aspect: gl.canvas.width / gl.canvas.height });
        }
        window.addEventListener("resize", resize, false);
        resize();

        const scene = new Transform();

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
                const { src_addr, dst_addr, bytes, timestamp } = data.packet;

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

                const trace = new Trace(gl, { from, to });
                scene.addChild(trace.mesh);

                await trace.fadeIn(200);
                await trace.fadeOut(200);
                scene.removeChild(trace.mesh);
            }
        };

        async function update(_now: number) {
            renderer.render({ scene, camera });
            orbit.update();
            requestAnimationFrame(update);
        }
        requestAnimationFrame(update);
    });

    function isActive(peer: Peer): boolean {
        return Date.now() - peer.last_message.getTime() < 1000 * 60;
    }
</script>

<div class="drawer drawer-open">
    <input id="peer-drawer" type="checkbox" class="drawer-toggle" checked={false} />

    <div class="drawer-content fixed">
        <canvas bind:this={canvas}></canvas>
    </div>

    <div class="drawer-side">
        <label for="peer-drawer" aria-label="close sidebar" class="drawer-overlay"></label>

        <aside class="flex flex-row bg-base-100/80 backdrop-blur-md max-h-screen w-full">
            <div class="is-drawer-close:w-0 overflow-x-scroll" onwheel={(e) => e.stopPropagation()} ontouchmove={(e) => e.stopPropagation()}>
                <div class="p-4 w-full flex flex-col gap-4">
                    <h2 class="text-xl font-bold">Peers</h2>
                </div>

                <table class="table">
                    <tbody>
                        {#each peers.toSorted((a, b) => {
                            const activeA = isActive(a) ? 1 : 0;
                            const activeB = isActive(b) ? 1 : 0;

                            if (activeA !== activeB) return activeB - activeA;

                            return b.ingress_bytes + b.egress_bytes - (a.ingress_bytes + a.egress_bytes);
                        }) as peer}
                            <tr
                                class="hover:bg-base-300 cursor-pointer"
                                onclick={() => {
                                    const source = camera.position.clone().normalize();
                                    const target = toCartesian({ lat: peer.info.lat, lon: peer.info.lon }).normalize();

                                    const radius = camera.position.len();
                                    const axis = new Vec3().cross(source, target).normalize();
                                    const angle = Math.acos(source.dot(target));

                                    const start = performance.now();
                                    const duration = 1000;

                                    function update(now: number) {
                                        const elapsed = now - start;
                                        const progress = Math.min(elapsed / duration, 1);
                                        const eased = progress * (2 - progress);

                                        const q = new Quat().fromAxisAngle(axis, angle * eased);
                                        const current = source.clone().applyQuaternion(q).scale(radius);

                                        camera.position.copy(current);
                                        orbit.forcePosition();

                                        if (progress < 1) requestAnimationFrame(update);
                                    }

                                    requestAnimationFrame(update);
                                }}
                            >
                                <td>
                                    <p>{peer.addr}</p>
                                </td>
                                <td>
                                    <div class="badge badge-soft badge-success w-full badge-sm min-w-20">{formatBytes(peer.ingress_bytes)}</div>
                                </td>
                                <td>
                                    <div class="badge badge-soft badge-error w-full badge-sm min-w-20">{formatBytes(peer.egress_bytes)}</div>
                                </td>
                                <td>
                                    {#if peer.info.source === "RegisteredCountry"}
                                        <svg
                                            xmlns="http://www.w3.org/2000/svg"
                                            height="24px"
                                            viewBox="0 -960 960 960"
                                            width="24px"
                                            class="fill-base-content"
                                            ><path
                                                d="M480-80q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 83-31.5 156T763-197q-54 54-127 85.5T480-80Zm-40-82v-78q-33 0-56.5-23.5T360-320v-40L168-552q-3 18-5.5 36t-2.5 36q0 121 79.5 212T440-162Zm276-102q41-45 62.5-100.5T800-480q0-98-54.5-179T600-776v16q0 33-23.5 56.5T520-680h-80v80q0 17-11.5 28.5T400-560h-80v80h240q17 0 28.5 11.5T600-440v120h40q26 0 47 15.5t29 40.5Z"
                                            /></svg
                                        >
                                    {:else if peer.info.source === "City"}
                                        <svg
                                            xmlns="http://www.w3.org/2000/svg"
                                            height="24px"
                                            viewBox="0 -960 960 960"
                                            width="24px"
                                            class="fill-base-content"
                                            ><path
                                                d="M480-80q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q152 0 263.5 98T876-538q-20-10-41.5-15.5T790-560q-19-73-68.5-130T600-776v16q0 33-23.5 56.5T520-680h-80v80q0 17-11.5 28.5T400-560h-80v80h240q11 0 20.5 5.5T595-459q-17 27-26 57t-9 62q0 63 32.5 117T659-122q-41 20-86 31t-93 11Zm-40-82v-78q-33 0-56.5-23.5T360-320v-40L168-552q-3 18-5.5 36t-2.5 36q0 121 79.5 212T440-162Zm340 82q-7 0-12-4t-7-10q-11-35-31-65t-43-59q-21-26-34-57t-13-65q0-58 41-99t99-41q58 0 99 41t41 99q0 34-13.5 64.5T873-218q-23 29-43 59t-31 65q-2 6-7 10t-12 4Zm0-113q10-17 22-31.5t23-29.5q14-19 24.5-40.5T860-340q0-33-23.5-56.5T780-420q-33 0-56.5 23.5T700-340q0 24 10.5 45.5T735-254q12 15 23.5 29.5T780-193Zm0-97q-21 0-35.5-14.5T730-340q0-21 14.5-35.5T780-390q21 0 35.5 14.5T830-340q0 21-14.5 35.5T780-290Z"
                                            /></svg
                                        >
                                    {:else if peer.info.source === "Manual"}
                                        <svg
                                            xmlns="http://www.w3.org/2000/svg"
                                            height="24px"
                                            viewBox="0 -960 960 960"
                                            width="24px"
                                            class="fill-base-content"
                                            ><path
                                                d="M440-42v-80q-125-14-214.5-103.5T122-440H42v-80h80q14-125 103.5-214.5T440-838v-80h80v80q125 14 214.5 103.5T838-520h80v80h-80q-14 125-103.5 214.5T520-122v80h-80Zm40-158q116 0 198-82t82-198q0-116-82-198t-198-82q-116 0-198 82t-82 198q0 116 82 198t198 82Zm0-120q-66 0-113-47t-47-113q0-66 47-113t113-47q66 0 113 47t47 113q0 66-47 113t-113 47Zm0-80q33 0 56.5-23.5T560-480q0-33-23.5-56.5T480-560q-33 0-56.5 23.5T400-480q0 33 23.5 56.5T480-400Zm0-80Z"
                                            /></svg
                                        >
                                    {/if}
                                </td>
                                <td>
                                    {formatFlag(peer.info.country_code)}
                                </td>
                                <td>
                                    {#if isActive(peer)}
                                        <div aria-label="success" class="status status-success animate-pulse"></div>
                                    {:else}
                                        <div aria-label="error" class="status status-error"></div>
                                    {/if}
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>

            <label for="peer-drawer" class="btn btn-ghost drawer-button is-drawer-open:rotate-y-180 flex-grow h-screen">
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    stroke-linejoin="round"
                    stroke-linecap="round"
                    stroke-width="2"
                    fill="none"
                    stroke="currentColor"
                    class="inline-block size-4 my-1.5"
                    ><path d="M4 4m0 2a2 2 0 0 1 2 -2h12a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2z"></path><path d="M9 4v16"></path><path
                        d="M14 10l2 2l-2 2"
                    ></path></svg
                >
            </label>
        </aside>
    </div>
</div>

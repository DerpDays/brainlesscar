import "./styles.css";

import { WebViewer } from "@rerun-io/web-viewer";

const address = "0.0.0.0";

const rerun_url = `ws://${address}:4000/rerun`;
const viewerElement = document.getElementById("app");

const command_url = `ws://${address}:4000/command`;
/** @type {WebSocket | null} */
let command_socket = null;
const mic_button = document.getElementById("microphone");
const mic_failure = document.getElementById("mic-failure");

async function main() {
    const viewer = new WebViewer();
    await viewer.start(rerun_url, viewerElement, {
        width: "100%",
        height: "100%",
        hide_welcome_screen: true,
    });
    command_socket = new WebSocket(command_url);
    command_socket.onerror = () => alert("command websocket failed");
}

mic_button.addEventListener("click", async () => {
    if (command_socket == null) {
        alert("command websocket not initialised yet");
        return;
    }
    if (command_socket.readyState !== WebSocket.OPEN) {
        command_socket = new WebSocket(command_url);
        alert("command websocket not initialised yet, trying to reconnect");
        return;
    }

    const stream = await navigator.mediaDevices
        .getUserMedia({ audio: true })
        .catch(() => {
            mic_failure.classList.add("visible");
        });

    const audioContext = new AudioContext();
    await audioContext.audioWorklet.addModule(
        new URL("./audio.js", import.meta.url),
    );

    const source = audioContext.createMediaStreamSource(stream);
    const workletNode = new AudioWorkletNode(
        audioContext,
        "microphone-processor",
    );

    source.connect(workletNode);
    workletNode.port.onmessage = (event) => {
        console.log("got message");
        command_socket.send(event.data);
    };
});

main();

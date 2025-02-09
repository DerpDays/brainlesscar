import "./styles.css";

import { WebViewer } from "@rerun-io/web-viewer";

const micButton = document.getElementById("mic-button");
const micContainer = document.getElementById("mic-container");
const transcript = document.getElementById("transcript");
const micIcon = document.getElementById("mic-icon");
let isOpen = false;

micButton.addEventListener("click", () => {
    navigator.mediaDevices
        .getUserMedia({ audio: true })
        .then(() => {
            isOpen = !isOpen;
            if (isOpen) {
                micContainer.classList.add("w-64");
                transcript.classList.remove("opacity-0");
                transcript.textContent = "Listening...";
                micIcon.innerHTML =
                    '<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />';
            } else {
                micContainer.classList.remove("w-64");
                transcript.classList.add("opacity-0");
                transcript.textContent = "Listening...";
                micIcon.innerHTML =
                    '<path stroke-linecap="round" stroke-linejoin="round" d="M12 18v3m0 0h3m-3 0H9m3-6a3 3 0 01-3-3V7a3 3 0 016 0v5a3 3 0 01-3 3z" />';
            }
        })
        .catch((err) => {
            alert("Microphone permission denied!");
            console.error("Error accessing microphone:", err);
        });
});

const rrdUrl = `ws://${window.location.host}/rerun`;
const parentElement = document.getElementById("app");

async function main() {
    const viewer = new WebViewer();
    await viewer.start(rrdUrl, parentElement, {
        width: "100%",
        height: "100%",
        hide_welcome_screen: true,
    });
}

main();

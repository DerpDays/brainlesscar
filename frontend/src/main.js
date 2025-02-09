import "./styles.css";

import { WebViewer } from "@rerun-io/web-viewer";

const rrdUrl = `ws://${window.location.host}/rerun`;
const viewerElement = document.getElementById("app");

async function main() {
    const viewer = new WebViewer();
    await viewer.start(rrdUrl, viewerElement, {
        width: "100%",
        height: "100%",
        hide_welcome_screen: true,
    });
}

main();

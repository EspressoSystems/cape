// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

let webSocket;
let clientId;
// See https://developer.mozilla.org/en-US/docs/Web/API/CloseEvent#status_codes
const webSocketNormalClose = 1000;

// Is the web socket connection open?
function isWebSocketOpen() {
  return webSocket && webSocket.readyState === webSocket.OPEN;
}

function sleep(ms) {
  new Promise((resolve) => setTimeout(resolve, ms));
}
// Attempt a web socket connection.
async function webSocketConnect() {
  if (!isWebSocketOpen()) {
    let protocol = window.location.protocol === "https:" ? "wss" : "ws";
    let host = window.location.host;
    let pathname = window.location.pathname;
    let url = `${protocol}://${host}${pathname}`;
    if (clientId) {
      url += `?c=${clientId}`;
    }
    console.log(`ws.js:webSocketConnect url:${url}`);
    webSocket = new WebSocket(url);
    webSocket.onmessage = webSocketOnMessage;
    webSocket.onopen = webSocketOnOpen;
    status("Web Socket enabled.");
    const SLEEP_MS = 50;
    for (let i = 1; i <= 10; ++i) {
      await sleep(SLEEP_MS);
      if (isWebSocketOpen()) {
        console.log("WebSocket connection opened after " + SLEEP_MS * i + "ms");
        break;
      }
    }
  }
}

// Send a Web Sockets message to the server.
async function webSocketSend(msg) {
  if (!isWebSocketOpen()) {
    await webSocketConnect();
  }
  if (isWebSocketOpen()) {
    status("webSocket: " + msg);
    webSocket.send(msg);
  } else {
    // TODO !corbett Happens when browsing to http://127.0.0.1:8080/
    status("Unable to open WebSocket connection.");
  }
}

// Append a message to the web page status text and console.
function status(msg) {
  console.log(`ws.js:status:${msg}`);
  let ta = document.getElementById("events");
  if (ta.value.length) {
    ta.value += "\n";
  }
  ta.value += new Date().toUTCString() + ":" + msg;
  ta.scrollTop = ta.scrollHeight;
}

// Generate a clean URL from the form field values.
function cleanUrl() {
  let raddr = document.getElementById("raddr").value;
  let amt = document.getElementById("amt").value;
  return `/transfer/${clientId}/${raddr}/${amt}`;
}

// Redirect to a clean URL.
function redirectClean() {
  console.log("redirectClean");
  window.location = cleanUrl();
  return false;
}

// React to a Web Sockets message from the server.
function webSocketOnMessage(msg) {
  const data = JSON.parse(msg.data);
  status(`Got webSocket message: clientId:${data.clientId} msg:${data.msg}.`);
}

function webSocketOnOpen(event) {
  console.log("got WebSocket.onopen event: " + JSON.stringify(event));
}

// Do a thing when the UI "Send" button is pressed.
function onSend() {
  status("Web Socket Send button pressed");
  webSocket.send("Web Socket Send button pressed:" + cleanUrl());
}

// After the page is loaded (but possibly before all the assets have loaded),
// open the Web Sockets connection and configure a message handler.
document.addEventListener("DOMContentLoaded", function () {
  console.log("ws.js:Adding event listener for DOMContentLoaded");

  webSocketConnect();

  webSocket.addEventListener("message", (message) => {
    const data = JSON.parse(message.data);
    console.log(`ws.js: message event listener: clientId:${data.clientId} msg:${data.msg}`);

    switch (data.cmd) {
      case "INIT":
        console.log(`ws.js:Server sent INIT for client ${data.clientId}`);
        clientId = data.clientId;
        break;
      case "LEAVE":
        console.log("ws.js:leave");
        alert("ws.js:Got LEAVE");
        webSocket.close(1000, "Received a LEAVE command.");
        webSocket = undefined;
        break;
      case "TRANSFER":
        // TODO !corbett Currently unused.
        status("TRANSFER " + data.xfr);
        break;
      default:
        console.log("ws.js: Received a message without a cmd field.");
    }
  });

  console.log("Pathname: " + window.location.pathname);
  let re = /\/transfer\/([^/]+)\/([^/]+)\/(.*)/;
  let ma = re.exec(window.location.pathname);
  if (ma) {
    clientId = ma[1];
    document.getElementById("saddr").value = ma[1];
    document.getElementById("raddr").value = ma[2];
    document.getElementById("amt").value = ma[3];
  }
});

// Arrange a graceful shutdown of the Web Sockets connection when
// the brower window closes. It's not clear this makes any difference
// or works across all browsers.
window.addEventListener("beforeunload", function (event) {
  webSocketSend(`LEAVE:${clientId}`);
});

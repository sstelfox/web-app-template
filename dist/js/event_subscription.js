// todo need to make sure this all gets run only once we fully load the page

// todo: build this dynmically based on hostname and scheme
const websocket = new WebSocket("ws://127.0.0.1:3000/events");

websocket.onopen = function() {
  console.log("event bus websocket connection opened");
  // if I need to send some message on initialization...
  //websocket.send(data);
}

websocket.onclose = function() {
  // todo: should handle automatic connection retrying...
  console.log("event bus websocket connection closed");
}

websocket.onmessage = function(event) {
  console.log(event);
}

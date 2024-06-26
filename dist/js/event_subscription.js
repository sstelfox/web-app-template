// todo need to make sure this all gets run only once we fully load the page

// todo: build this dynmically based on hostname and scheme
const websocket = new WebSocket("ws://127.0.0.1:3000/events");

websocket.onopen = function () {
  console.log("event bus websocket connection opened");
  // if I need to send some message on initialization...
  //websocket.send(data);
};

websocket.onclose = function () {
  // todo: should handle automatic connection retrying...
  console.log("event bus websocket connection closed");
};

websocket.onmessage = function (event) {
  console.log(event);

  const raw_data = event.data;
  const data = JSON.parse(raw_data);

  const base_row_node = document.createElement("tr");

  const event_type_col = document.createElement("td");
  const event_type_content = document.createTextNode(data.event_type);
  event_type_col.appendChild(event_type_content);
  base_row_node.appendChild(event_type_col);

  const size_col = document.createElement("td");
  const size_content = document.createTextNode(data.payload.length);
  size_col.appendChild(size_content);
  base_row_node.appendChild(size_col);

  const payload_col = document.createElement("td");
  const payload_content = document.createTextNode(data.payload);
  payload_col.appendChild(payload_content);
  base_row_node.appendChild(payload_col);

  const decoded_col = document.createElement("td");

  if (data.decoded) {
    const decoded_content = document.createTextNode(
      JSON.stringify(data.decoded),
    );
    decoded_col.appendChild(decoded_content);
  }

  base_row_node.appendChild(decoded_col);

  const event_list = document.getElementById("event-list");
  event_list.appendChild(base_row_node);
};

const test_event_btn = document.getElementById("test-event");
if (test_event_btn) {
  test_event_btn.addEventListener("click", function (event) {
    event.preventDefault();

    fetch("/events/test")
      .then(function (response) {
        if (!response.ok) {
          return response.json().then(function (responseJson) {
            throw new Error(responseJson);
          });
        }
      })
      .catch(function (error) {
        console.log("creating test event failed: ", error);
      });
  });
}

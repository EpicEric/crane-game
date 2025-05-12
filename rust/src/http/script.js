document.addEventListener("htmx:wsClose", (e) => {
  const notification = document.querySelector("#notification");
  notification.classList.add("htmx-added");
  const notificationState = document.querySelector("#notification-state");
  notificationState.classList.replace("hide", "show");
  notification.innerText = "Disconnected."
  notification.classList.remove("htmx-added");
});
function getTimestamp() {
  return performance.now() | 0;
}
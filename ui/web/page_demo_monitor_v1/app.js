const timelineEl = document.getElementById("timeline");
const counterEl = document.getElementById("event-counter");
const template = document.getElementById("event-template");
const btnNext = document.getElementById("btn-next");
const btnReset = document.getElementById("btn-reset");

const seedEvents = [
  {
    type: "user",
    title: "用户消息",
    summary: "帮我分析最近 7 天最慢的测试并给优化建议。",
    status: "success",
    time: "11:58:12.220"
  },
  {
    type: "assistant",
    title: "Agent 开始处理",
    summary: "已收到请求，准备读取 CI 报告并执行测试统计。",
    status: "running",
    time: "11:58:12.631"
  },
  {
    type: "tool",
    title: "Tool Call: fetch_ci_artifacts",
    summary: "下载最近 7 天的流水线产物与测试报告索引。",
    status: "success",
    time: "11:58:13.002",
    code: "tool.fetch_ci_artifacts({ window_days: 7, include_flaky: true })"
  },
  {
    type: "shell",
    title: "Shell 执行: 聚合测试时长",
    summary: "运行脚本聚合 slow tests。",
    status: "running",
    time: "11:58:14.145",
    code: "zsh -lc \"python3 scripts/slow_tests.py --days 7 --top 10\"",
    output:
      "collecting reports ...\nfound 142 test files\nslowest:\n1) test_api_streaming.py::test_reconnect  18.4s\n2) m6_submit_stream.rs::test_timeout_path 14.9s"
  },
  {
    type: "tool",
    title: "Approval Requested: shell.write",
    summary: "需要写入临时 profiling 文件，等待用户审批。",
    status: "waiting",
    time: "11:58:14.370",
    code: "tool.approval_request({ action: 'shell.write', path: '/tmp/profile.json' })"
  },
  {
    type: "shell",
    title: "Shell 执行完成",
    summary: "命令执行成功，已生成优化建议草稿。",
    status: "success",
    time: "11:58:16.521",
    output:
      "done in 2.31s\nrecommendations:\n- cache rust target per branch\n- split integration tests by feature flag\n- add retry for flaky network tests"
  },
  {
    type: "assistant",
    title: "Assistant Message",
    summary: "已整理慢测列表和优化建议，准备输出到会话。",
    status: "success",
    time: "11:58:16.924"
  }
];

let cursor = 0;
let rendered = [];

function render() {
  timelineEl.innerHTML = "";
  rendered.forEach((event, idx) => {
    const node = template.content.firstElementChild.cloneNode(true);
    node.classList.add(`type-${event.type}`);
    node.style.setProperty("--item-index", String(idx));

    node.querySelector(".event-time").textContent = event.time;
    node.querySelector(".event-title").textContent = event.title;
    node.querySelector(".event-summary").textContent = event.summary;

    const status = node.querySelector(".status-badge");
    status.textContent = event.status;
    status.classList.add(`status-${event.status}`);

    const code = node.querySelector(".event-code");
    if (event.code) {
      code.textContent = event.code;
      code.classList.add("show");
    }

    const outputWrap = node.querySelector(".event-output-wrap");
    const output = node.querySelector(".event-output");
    if (event.output) {
      output.textContent = event.output;
      outputWrap.classList.add("show");
      output.classList.add("show");
    }

    timelineEl.appendChild(node);
  });

  counterEl.textContent = `events: ${rendered.length}`;
  timelineEl.parentElement.scrollTo({ top: timelineEl.parentElement.scrollHeight, behavior: "smooth" });
}

function pushNext() {
  if (cursor >= seedEvents.length) {
    return;
  }
  rendered.push(seedEvents[cursor]);
  cursor += 1;
  render();
}

btnNext.addEventListener("click", pushNext);
btnReset.addEventListener("click", () => {
  cursor = 0;
  rendered = [];
  render();
});

for (let i = 0; i < 4; i += 1) {
  pushNext();
}

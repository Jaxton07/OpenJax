const streamEl = document.getElementById("step-stream");
const summaryEl = document.getElementById("assistant-summary");
const template = document.getElementById("step-template");

const steps = [
  {
    type: "think",
    title: "分析任务",
    subtitle: "Thought",
    description: "拆分为报告采集、慢测聚合、优化建议输出三个阶段。",
    status: "running",
    time: "12:07:08.201",
    expanded: false
  },
  {
    type: "tool",
    title: "cargo.toml",
    subtitle: "openjax-core/Cargo.toml",
    delta: "+1 -1",
    actionLabel: "查看变更",
    description: "获取最近 7 天的 CI 产物索引与测试报告清单。",
    status: "success",
    time: "12:07:08.622",
    code: "tool.fetch_ci_artifacts({ window_days: 7, include_flaky: true })",
    expanded: false
  },
  {
    type: "shell",
    title: "openJax 白名单运行",
    subtitle: "zsh",
    actionLabel: "终端",
    description: "统计最慢测试用例并识别可拆分维度。",
    status: "running",
    time: "12:07:09.013",
    code: "zsh -lc \"python3 scripts/slow_tests.py --days 7 --top 8 --by-suite\"",
    output:
      "collecting reports ...\nfound 142 test files\nslowest suites:\n- gateway_integration: 95.3s\n- core_stream_submit: 74.1s\n- tui_navigation: 51.7s",
    expanded: false
  },
  {
    type: "tool",
    title: "logger.rs",
    subtitle: "openjax-core/src/logger.rs",
    delta: "+8 -0",
    actionLabel: "查看变更",
    description: "将聚合结果写入临时文件需要审批。",
    status: "waiting",
    time: "12:07:09.460",
    code: "tool.approval_request({ action: 'shell.write', path: '/tmp/slow_test_report.json' })",
    expanded: false
  },
  {
    type: "shell",
    title: "1 Lint Error",
    subtitle: "Clippy",
    description: "审批通过后命令继续执行并生成建议。",
    status: "failed",
    time: "12:07:11.984",
    output:
      "done in 2.9s\npriority actions:\n1. split gateway_integration by api group\n2. cache cargo target per branch\n3. isolate flaky network tests",
    expanded: false
  },
  {
    type: "think",
    title: "整理回答",
    subtitle: "Thought",
    description: "按收益/改造成本排序，准备输出给用户。",
    status: "success",
    time: "12:07:12.201",
    expanded: false
  }
];

function renderStep(step) {
  const node = template.content.firstElementChild.cloneNode(true);
  node.classList.add(`type-${step.type}`);
  if (step.expanded) {
    node.classList.add("expanded");
  }

  const head = node.querySelector(".step-head");
  const title = node.querySelector(".step-title");
  const subtitle = node.querySelector(".step-subtitle");
  const time = node.querySelector(".step-time");
  const delta = node.querySelector(".step-delta");
  const action = node.querySelector(".step-action");
  const status = node.querySelector(".step-status");
  const desc = node.querySelector(".step-desc");
  const code = node.querySelector(".step-code");
  const outputWrap = node.querySelector(".step-output-wrap");
  const output = node.querySelector(".step-output");

  title.textContent = step.title;
  subtitle.textContent = step.subtitle ?? "";
  time.textContent = step.time;
  delta.textContent = step.delta ?? "";
  action.textContent = step.actionLabel ?? "详情";
  status.textContent = step.status;
  node.classList.add(`status-${step.status}`);
  status.classList.add(`status-${step.status}`);
  desc.textContent = step.description;

  if (!step.delta) {
    delta.style.display = "none";
  }

  if (!step.actionLabel) {
    action.style.display = "none";
  }

  if (step.code) {
    code.textContent = step.code;
  } else {
    code.remove();
  }

  if (step.output) {
    output.textContent = step.output;
  } else {
    outputWrap.remove();
  }

  head.addEventListener("click", () => {
    toggleStep(node);
  });

  const body = node.querySelector(".step-body");
  if (step.expanded) {
    node.classList.add("expanded");
    body.style.height = "auto";
    body.style.opacity = "1";
  } else {
    node.classList.remove("expanded");
    body.style.height = "0px";
    body.style.opacity = "0";
  }

  return node;
}

function toggleStep(node) {
  const body = node.querySelector(".step-body");
  const expanded = node.classList.contains("expanded");

  if (expanded) {
    collapseStep(node, body);
  } else {
    expandStep(node, body);
  }
}

function expandStep(node, body) {
  if (body.dataset.animating === "1") {
    return;
  }
  body.dataset.animating = "1";
  const inner = body.querySelector(".step-body-inner");
  if (!inner) {
    node.classList.add("expanded");
    body.style.height = "auto";
    body.style.opacity = "1";
    body.dataset.animating = "0";
    return;
  }

  node.classList.add("expanded");
  body.style.height = "0px";
  body.style.opacity = "1";
  // Force layout to ensure the next height write triggers transition.
  void body.offsetHeight;
  const targetHeight = inner.scrollHeight;
  requestAnimationFrame(() => {
    body.style.height = `${targetHeight}px`;
  });

  const fallback = window.setTimeout(() => {
    body.style.height = "auto";
    body.dataset.animating = "0";
  }, 280);
  const onEnd = (event) => {
    if (event.propertyName !== "height") {
      return;
    }
    window.clearTimeout(fallback);
    body.style.height = "auto";
    body.dataset.animating = "0";
    body.removeEventListener("transitionend", onEnd);
  };
  body.addEventListener("transitionend", onEnd);
}

function collapseStep(node, body) {
  if (body.dataset.animating === "1") {
    return;
  }
  body.dataset.animating = "1";
  const inner = body.querySelector(".step-body-inner");
  if (!inner) {
    node.classList.remove("expanded");
    body.style.height = "0px";
    body.style.opacity = "0";
    body.dataset.animating = "0";
    return;
  }

  const startHeight = body.scrollHeight || inner.scrollHeight;
  body.style.height = `${startHeight}px`;
  body.style.opacity = "1";
  void body.offsetHeight;
  requestAnimationFrame(() => {
    body.style.height = "0px";
    body.style.opacity = "0";
  });

  const fallback = window.setTimeout(() => {
    node.classList.remove("expanded");
    body.dataset.animating = "0";
  }, 280);
  const onEnd = (event) => {
    if (event.propertyName !== "height") {
      return;
    }
    window.clearTimeout(fallback);
    node.classList.remove("expanded");
    body.dataset.animating = "0";
    body.removeEventListener("transitionend", onEnd);
  };
  body.addEventListener("transitionend", onEnd);
}

function renderAll() {
  streamEl.innerHTML = "";
  for (const step of steps) {
    streamEl.appendChild(renderStep(step));
  }
  summaryEl.classList.add("hidden");
  summaryEl.textContent = "";
}

renderAll();

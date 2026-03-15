interface SettingsSidebarProps {
  activeTab: "general" | "provider";
  onChangeTab: (tab: "general" | "provider") => void;
}

export default function SettingsSidebar(props: SettingsSidebarProps) {
  return (
    <aside className="settings-sidebar-nav">
      <button
        className={props.activeTab === "general" ? "active" : ""}
        onClick={() => props.onChangeTab("general")}
      >
        通用设置
      </button>
      <button
        className={props.activeTab === "provider" ? "active" : ""}
        onClick={() => props.onChangeTab("provider")}
      >
        LLM Provider
      </button>
    </aside>
  );
}

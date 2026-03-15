interface SettingsSidebarProps {
  activeTab: "general" | "provider";
  onChangeTab: (tab: "general" | "provider") => void;
}

export default function SettingsSidebar(props: SettingsSidebarProps) {
  return (
    <aside className="settings-sidebar-nav" aria-label="设置分组导航">
      <button
        type="button"
        className={props.activeTab === "general" ? "settings-nav-item active" : "settings-nav-item"}
        aria-current={props.activeTab === "general" ? "page" : undefined}
        onClick={() => props.onChangeTab("general")}
      >
        通用设置
      </button>
      <button
        type="button"
        className={
          props.activeTab === "provider" ? "settings-nav-item active" : "settings-nav-item"
        }
        aria-current={props.activeTab === "provider" ? "page" : undefined}
        onClick={() => props.onChangeTab("provider")}
      >
        LLM Provider
      </button>
    </aside>
  );
}

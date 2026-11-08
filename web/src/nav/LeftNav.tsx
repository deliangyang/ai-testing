import type { NavCatalog } from './types'
import { PassRateBar } from './PassRateBar'
import { ValidationBadge } from './ValidationBadge'
import { reportRunDotClass, runStatusDotClass } from './tokens'
import './nav.css'

export interface LeftNavProps {
  catalog: NavCatalog
  collapsed: boolean
  onToggleCollapse: () => void
  onOpenScenario?: (scenarioId: string) => void
  onOpenReport?: (reportId: string) => void
}

export function LeftNav({
  catalog,
  collapsed,
  onToggleCollapse,
  onOpenScenario,
  onOpenReport,
}: LeftNavProps) {
  return (
    <>
      <button
        type="button"
        className="app-shell__nav-toggle"
        aria-expanded={!collapsed}
        aria-label={collapsed ? '展开左导航' : '折叠左导航'}
        onClick={onToggleCollapse}
      >
        {collapsed ? '»' : '«'}
      </button>
      <div
        className={`left-nav${collapsed ? ' left-nav--collapsed' : ''}`}
        data-testid="left-nav-panel"
      >
        <section className="left-nav__section" aria-label="场景库">
          <h2 className="left-nav__heading">场景库</h2>
          <ul className="left-nav__list" data-testid="nav-scenarios">
            {catalog.scenarios.map((item) => (
              <li
                key={item.id}
                className="left-nav__item"
                data-testid={`nav-scenario-${item.id}`}
                onDoubleClick={() => {
                  onOpenScenario?.(item.id)
                }}
              >
                <span
                  className={`nav-status-dot ${runStatusDotClass(item.lastRunStatus)}`}
                  aria-hidden
                  data-testid={`nav-status-${item.id}`}
                />
                <span className="left-nav__label">{item.title}</span>
                {!collapsed ? <PassRateBar rate={item.passRate} /> : null}
              </li>
            ))}
          </ul>
        </section>

        <section className="left-nav__section" aria-label="计划库">
          <h2 className="left-nav__heading">计划库</h2>
          <ul className="left-nav__list" data-testid="nav-plans">
            {catalog.plans.map((item) => (
              <li key={item.id} className="left-nav__item" data-testid={`nav-plan-${item.id}`}>
                <ValidationBadge status={item.gateStatus} />
                <span className="left-nav__label">{item.title}</span>
              </li>
            ))}
          </ul>
        </section>

        <section className="left-nav__section" aria-label="报告">
          <h2 className="left-nav__heading">报告</h2>
          <ul className="left-nav__list" data-testid="nav-reports">
            {catalog.reports.map((item) => (
              <li
                key={item.id}
                className="left-nav__item"
                data-testid={`nav-report-${item.id}`}
                onDoubleClick={() => {
                  onOpenReport?.(item.id)
                }}
              >
                <span className={`nav-status-dot ${reportRunDotClass(item.runStatus)}`} />
                <span className="left-nav__label">{item.title}</span>
              </li>
            ))}
          </ul>
        </section>
      </div>
    </>
  )
}

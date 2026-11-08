import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react'

import type { NavCatalog } from '../nav/types'
import {
  buildGlobalSearchItems,
  filterGlobalSearchItems,
  type GlobalSearchItem,
} from './globalSearch'

export interface GlobalSearchDialogProps {
  open: boolean
  catalog: NavCatalog
  onClose: () => void
  onPick: (item: GlobalSearchItem) => void
}

export function GlobalSearchDialog({ open, catalog, onClose, onPick }: GlobalSearchDialogProps) {
  const [query, setQuery] = useState('')
  const [highlight, setHighlight] = useState(0)
  const [prevOpen, setPrevOpen] = useState(open)
  const [prevQuery, setPrevQuery] = useState(query)
  const inputRef = useRef<HTMLInputElement>(null)

  const items = useMemo(() => {
    const all = buildGlobalSearchItems(catalog)
    return filterGlobalSearchItems(all, query)
  }, [catalog, query])

  if (prevOpen !== open) {
    setPrevOpen(open)
    if (!open) {
      setQuery('')
      setHighlight(0)
    }
  }
  if (prevQuery !== query) {
    setPrevQuery(query)
    setHighlight(0)
  }

  useEffect(() => {
    if (!open) {
      return
    }
    const id = window.requestAnimationFrame(() => {
      inputRef.current?.focus()
    })
    return () => {
      window.cancelAnimationFrame(id)
    }
  }, [open])

  if (!open) {
    return null
  }

  const pick = (item: GlobalSearchItem) => {
    onPick(item)
    onClose()
  }

  const onDialogKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      onClose()
      return
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      setHighlight((i) => Math.min(i + 1, Math.max(items.length - 1, 0)))
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      setHighlight((i) => Math.max(i - 1, 0))
      return
    }
    if (event.key === 'Enter' && items[highlight]) {
      event.preventDefault()
      pick(items[highlight])
    }
  }

  return (
    <div
      className="shell-dialog-backdrop"
      data-testid="global-search-backdrop"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="shell-dialog global-search-dialog"
        data-testid="global-search-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="全局搜索"
        onClick={(event) => {
          event.stopPropagation()
        }}
        onKeyDown={onDialogKeyDown}
      >
        <input
          ref={inputRef}
          className="global-search-dialog__input"
          data-testid="global-search-input"
          type="search"
          placeholder="搜索场景或报告…"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value)
          }}
        />
        <ul className="global-search-dialog__list" data-testid="global-search-results">
          {items.length === 0 ? (
            <li className="global-search-dialog__empty">无匹配结果</li>
          ) : (
            items.map((item, index) => (
              <li key={`${item.kind}:${item.refId}`}>
                <button
                  type="button"
                  className={
                    index === highlight
                      ? 'global-search-dialog__item global-search-dialog__item--active'
                      : 'global-search-dialog__item'
                  }
                  onMouseEnter={() => {
                    setHighlight(index)
                  }}
                  onClick={() => {
                    pick(item)
                  }}
                >
                  <span className="global-search-dialog__title">{item.title}</span>
                  <span className="global-search-dialog__meta">{item.subtitle}</span>
                </button>
              </li>
            ))
          )}
        </ul>
      </div>
    </div>
  )
}

import { useId } from 'react'

import { splitTemplateSegments } from './templateHighlight'

export interface TemplateFieldProps {
  label: string
  value: string
  suggestions: string[]
  testId: string
  onChange: (value: string) => void
}

export function TemplateField({ label, value, suggestions, testId, onChange }: TemplateFieldProps) {
  const listId = useId()
  const segments = splitTemplateSegments(value)

  return (
    <label className="node-property__field template-field">
      <span className="node-property__label">{label}</span>
      <div className="template-field__wrap">
        <div className="template-field__mirror" aria-hidden data-testid={`${testId}-mirror`}>
          {segments.length === 0 ? (
            <span className="template-field__text">&nbsp;</span>
          ) : (
            segments.map((seg, i) => (
              <span
                key={`${String(i)}-${seg.text}`}
                className={seg.kind === 'var' ? 'template-field__var' : 'template-field__text'}
              >
                {seg.text}
              </span>
            ))
          )}
        </div>
        <input
          list={listId}
          data-testid={testId}
          className="template-field__input"
          type="text"
          value={value}
          onChange={(e) => {
            onChange(e.target.value)
          }}
        />
        <datalist id={listId}>
          {suggestions.map((item) => (
            <option key={item} value={item} />
          ))}
        </datalist>
      </div>
      {suggestions.length > 0 ? (
        <ul className="template-field__suggestions" data-testid={`${testId}-suggestions`}>
          {suggestions.map((item) => (
            <li key={item}>
              <code>{item}</code>
            </li>
          ))}
        </ul>
      ) : null}
    </label>
  )
}

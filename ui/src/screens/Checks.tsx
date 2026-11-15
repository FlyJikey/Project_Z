import { Card, PageTitle } from '../components/ui.tsx'
import { t } from '../strings/ru.ts'

export function Checks() {
  return (
    <>
      <PageTitle>{t.checks.title}</PageTitle>
      <Card title={t.checks.soon}>
        <p className="text-sm text-muted">{t.checks.hint}</p>
      </Card>
    </>
  )
}

import { PageHeader, Section } from '../../components/common';

export function AboutPage() {
  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="About" />
      <Section title="About">
        <h3 className="text-lg font-semibold text-white">Song Battle</h3>
        <p className="mt-1 text-sm text-white/60">Music tournaments decided by your Kick chat.</p>
        <dl className="mt-4 grid grid-cols-2 gap-3 text-sm">
          <dt className="text-white/50">Version</dt>
          <dd className="text-white/80">0.1.0</dd>
          <dt className="text-white/50">Stack</dt>
          <dd className="text-white/80">Tauri · React · Rust</dd>
        </dl>
      </Section>
    </div>
  );
}

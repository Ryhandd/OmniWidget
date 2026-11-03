import { DropZone } from './components/DropZone';
import { JobStatus } from './components/JobStatus';
import { OutputFolder } from './components/OutputFolder';
import { TargetPicker } from './components/TargetPicker';
import { actions, useConverter } from './controller';
import { isStagedPath } from './intake';
import { canConvert, isActive } from './state';
import { useFileIntake } from './useFileIntake';
import { useOutputPrefs } from './useOutputPrefs';
import './styles.css';

export default function ConverterModule() {
  const s = useConverter();
  const out = useOutputPrefs();
  const active = isActive(s);
  const staged = isStagedPath(s.filePath);

  // Dialog, drag & drop, dan paste bermuara ke satu jalur: path file -> actions.loadFile.
  const intake = useFileIntake(!active, (path) => void actions.loadFile(path));

  return (
    <div class="conv">
      <DropZone
        filePath={s.filePath}
        source={s.source}
        over={intake.over}
        locked={active}
        staging={intake.staging}
        staged={staged}
        notice={intake.notice}
        dropProps={intake.dropProps}
        onBrowse={intake.browse}
      />

      {s.targets.length > 0 && (
        <>
          <TargetPicker
            source={s.source}
            targets={s.targets}
            target={s.target}
            pdfPages={s.pdfPages}
            locked={active}
            canConvert={canConvert(s)}
            active={active}
            cancelling={s.cancelling}
            onTarget={actions.setTarget}
            onPages={actions.setPages}
            onConvert={() => void actions.convert(out.outputDir)}
            onCancel={actions.cancel}
          />
          <OutputFolder prefs={out.prefs} locked={active} staged={staged} onPick={out.pickFolder} onUseSource={out.chooseSource} />
        </>
      )}

      <JobStatus state={s} />
    </div>
  );
}

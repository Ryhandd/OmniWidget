import { render } from 'preact';
import './themes/tailwind.css';
import './themes/base.css';
import { App } from './shell/App';
import { loadSavedTheme } from './themes/theme';
import { startActivityReporter } from './core/activity';
import { startIndicatorBridge } from './core/indicators';
import { startPomodoroBridge } from './core/pomodoro';

startActivityReporter();
startIndicatorBridge();
startPomodoroBridge(); // hitung mundur Pomodoro di pill (timer aslinya di Rust)

loadSavedTheme().finally(() => {
  render(<App />, document.getElementById('app')!);
});

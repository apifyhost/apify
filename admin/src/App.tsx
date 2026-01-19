import { BrowserRouter } from 'react-router-dom';
import { Application } from './core/Application';

function App() {
  return (
    <BrowserRouter basename="/admin">
      <Application />
    </BrowserRouter>
  );
}

export default App;

import { BrowserRouter, Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import Dashboard from './pages/Dashboard';
import Rules from './pages/Rules';
import Simulator from './pages/Simulator';
import Secrets from './pages/Secrets';
import AuditLogs from './pages/AuditLogs';

export default function App() {
  return (
    <BrowserRouter>
      <Layout>
        <Routes>
          <Route path="/" element={<Dashboard />} />
          <Route path="/rules" element={<Rules />} />
          <Route path="/simulator" element={<Simulator />} />
          <Route path="/secrets" element={<Secrets />} />
          <Route path="/audit" element={<AuditLogs />} />
        </Routes>
      </Layout>
    </BrowserRouter>
  );
}

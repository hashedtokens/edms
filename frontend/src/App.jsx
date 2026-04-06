import React from 'react';
import { HashRouter as Router, Routes, Route } from 'react-router-dom';
import { HomePage } from './components/pages/HomePage';
import { ListView } from './components/pages/ListView';
import { TestView } from './components/pages/TestView';
import './index.css';

function App() {
  return (
    <Router>
      <Routes>
        <Route path="/" element={<HomePage />} />
        <Route path="/testview" element={<TestView />} />
        <Route path="/list" element={<ListView />} />
      </Routes>
    </Router>
  );
}

export default App;
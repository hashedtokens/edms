import React from 'react';

export const JsonEditor = ({ 
  title, 
  value, 
  onChange, 
  readOnly = false, 
  height = '288px',
  showActions = true 
}) => {
  const byteSize = new Blob([value]).size;

  return (
    <div className="border border-gray-300 rounded-lg p-4 bg-white shadow-sm">
      <div className="flex justify-between items-center mb-3">
        <h2 className="text-gray-800 font-semibold uppercase">{title}</h2>
        {showActions && (
          <div className="flex items-center gap-2">
            <span className="text-gray-600 text-sm">{byteSize} B</span>
            <button className="p-2 bg-gray-100 border border-gray-300 rounded cursor-pointer hover:bg-gray-200">
              ⬇
            </button>
            <button 
              className="p-2 bg-gray-100 border border-gray-300 rounded cursor-pointer hover:bg-gray-200"
              onClick={() => navigator.clipboard.writeText(value)}
            >
              📋
            </button>
          </div>
        )}
      </div>
      {readOnly ? (
        <pre className="w-full border border-gray-300 rounded p-3 font-mono text-sm text-gray-700 bg-gray-50 overflow-auto whitespace-pre-wrap"
          style={{ height }}
        >
          {value}
        </pre>
      ) : (
        <textarea
          className="w-full border border-gray-300 rounded p-3 font-mono text-sm text-gray-700 bg-gray-50 resize-none outline-none focus:border-purple-500 focus:ring-2 focus:ring-purple-200"
          style={{ height }}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          spellCheck={false}
        />
      )}
    </div>
  );
};
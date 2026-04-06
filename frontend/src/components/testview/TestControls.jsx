import React from 'react';

export const TestControls = ({
  method,
  setMethod,
  url,
  setUrl,
  onTest,
  onStop,
  onSave,
  onDownload,
  onCopyUrl,
  isLoading
}) => {
  return (
    <div className="flex justify-end items-center mb-6">
      <div className="flex gap-3">
        <button 
          className="px-6 py-2 bg-gradient-to-r from-purple-600 to-pink-600 text-white border-none rounded font-semibold cursor-pointer uppercase hover:opacity-90 disabled:opacity-50"
          onClick={onTest}
          disabled={isLoading}
        >
          {isLoading ? 'Testing...' : 'Test'}
        </button>
        <button className="px-6 py-2 bg-white border border-gray-300 rounded font-semibold cursor-pointer uppercase hover:bg-gray-50">
          Stop
        </button>
        <button className="px-6 py-2 bg-white border border-gray-300 rounded font-semibold cursor-pointer uppercase hover:bg-gray-50">
          Save
        </button>
        <button className="px-6 py-2 bg-white border border-gray-300 rounded font-semibold cursor-pointer uppercase hover:bg-gray-50">
          Download
        </button>
      </div>
    </div>
  );
};
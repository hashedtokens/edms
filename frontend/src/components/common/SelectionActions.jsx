import React from 'react';

export const SelectionActions = ({ 
  selectedCount, 
  onRemoveAll, 
  onBulkTag, 
  onExport,
  showBulkActions = true 
}) => {
  if (selectedCount === 0) return null;

  return (
    <div className="px-5 py-3 bg-gray-50 border-b border-gray-200 flex justify-between items-center">
      <span className="font-semibold text-gray-700">
        {selectedCount} item(s) selected
      </span>
      <div className="flex gap-3">
        {showBulkActions && (
          <>
            <button 
              className="px-4 py-2 border border-green-600 bg-white text-green-600 rounded cursor-pointer text-sm hover:bg-green-600 hover:text-white"
              onClick={onBulkTag}
            >
              Bulk Tag
            </button>
            <button 
              className="px-4 py-2 border border-blue-600 bg-white text-blue-600 rounded cursor-pointer text-sm hover:bg-blue-600 hover:text-white"
              onClick={onExport}
            >
              Export Selected
            </button>
          </>
        )}
        <button 
          className="px-4 py-2 border border-red-600 bg-white text-red-600 rounded cursor-pointer text-sm hover:bg-red-600 hover:text-white"
          onClick={onRemoveAll}
        >
          Remove All Selections
        </button>
      </div>
    </div>
  );
};
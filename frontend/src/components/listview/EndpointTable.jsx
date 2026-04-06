import React from 'react';
import { EndpointRow } from './EndpointRow';

export const EndpointTable = ({
  endpoints,
  selectedEndpoints,
  toggleEndpoint,
  toggleSelectAll,
  expandedTags,
  toggleTags,
  onEndpointClick,
  getTotalRequestSize,
  getTotalResponseSize,
  filteredEndpoints
}) => {
  const allSelected = selectedEndpoints.length === filteredEndpoints.length && filteredEndpoints.length > 0;

  return (
    <div className="bg-white rounded-lg border border-gray-200 overflow-hidden">
      {/* Table Header - Fixed */}
      <div className="grid grid-cols-[50px,100px,250px,150px,280px,180px] bg-gray-50 border-b-2 border-gray-200 px-4 py-3 font-semibold text-sm text-gray-800 sticky top-0 z-10">
        <div className="flex items-center justify-center">
          <input 
            type="checkbox" 
            checked={allSelected}
            onChange={toggleSelectAll}
            className="cursor-pointer w-4 h-4"
          />
        </div>
        <div>Method</div>
        <div>Endpoint URL</div>
        <div>Tags</div>
        <div>Data Size</div>
        <div>Annotation</div>
        <div>Date & Time</div>
      </div>

      {/* Table Rows - Scrollable */}
      <div className="max-h-[calc(100vh-400px)] overflow-y-auto">
        {endpoints.map((endpoint) => (
          <EndpointRow
            key={endpoint.id}
            endpoint={endpoint}
            isSelected={selectedEndpoints.includes(endpoint.id)}
            isExpanded={expandedTags[endpoint.id]}
            onToggleSelect={() => toggleEndpoint(endpoint.id)}
            onToggleTags={() => toggleTags(endpoint.id)}
            onClick={() => onEndpointClick(endpoint.id)}
            getTotalRequestSize={getTotalRequestSize}
            getTotalResponseSize={getTotalResponseSize}
          />
        ))}
      </div>
    </div>
  );
};
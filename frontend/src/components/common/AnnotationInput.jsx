import React from 'react';

export const AnnotationInput = ({ 
  annotation, 
  setAnnotation, 
  placeholder = "Annotation per endpoint",
  maxWords = 250 
}) => {
  const wordCount = annotation.split(/\s+/).filter(w => w.length > 0).length;
  
  return (
    <div className="flex items-center gap-3 mb-4">
      <input
        type="text"
        placeholder={`${placeholder} (max ${maxWords} words)`}
        className="flex-1 px-4 py-2 border border-gray-300 rounded text-gray-700 outline-none focus:border-purple-500 focus:ring-2 focus:ring-purple-200"
        value={annotation}
        onChange={(e) => setAnnotation(e.target.value)}
      />
      <div className="flex flex-col items-end w-32">
        <span className="text-gray-600 font-medium">
          {wordCount} words
        </span>
        <div className="w-full bg-gray-200 rounded-full h-1 mt-1">
          <div 
            className="bg-blue-600 h-1 rounded-full transition-all duration-300"
            style={{ width: `${Math.min((wordCount / maxWords) * 100, 100)}%` }}
          ></div>
        </div>
      </div>
    </div>
  );
};
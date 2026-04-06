import React, { useState } from 'react';

export const TagManager = ({ 
  tags, 
  onAddTag, 
  onRemoveTag, 
  maxTags = 25,
  placeholder = "Add tags (press Enter or click +)"
}) => {
  const [currentTag, setCurrentTag] = useState('');

  const handleAddTag = () => {
    if (currentTag.trim() && tags.length < maxTags) {
      onAddTag(currentTag.trim());
      setCurrentTag('');
    }
  };

  const handleKeyPress = (e) => {
    if (e.key === 'Enter') {
      handleAddTag();
    }
  };

  return (
    <div className="mb-6">
      <div className="flex gap-3 mb-3">
        <input
          type="text"
          placeholder={placeholder}
          className="flex-1 px-4 py-2 border border-gray-300 rounded text-gray-700 outline-none focus:border-purple-500 focus:ring-2 focus:ring-purple-200"
          value={currentTag}
          onChange={(e) => setCurrentTag(e.target.value)}
          onKeyPress={handleKeyPress}
        />
        <button 
          className="px-6 py-2 bg-gradient-to-r from-purple-600 to-pink-600 text-white border-none rounded font-bold cursor-pointer text-lg hover:opacity-90 disabled:opacity-50"
          onClick={handleAddTag}
          disabled={tags.length >= maxTags}
        >
          +
        </button>
      </div>
      <div className="flex flex-wrap gap-2 items-center min-h-8">
        {tags.map((tag, index) => (
          <span key={index} className="px-3 py-1 bg-purple-100 text-purple-700 border border-purple-200 rounded text-sm flex items-center gap-2 animate-tagAppear">
            {tag}
            <button 
              className="text-purple-700 text-lg hover:text-red-600 transition-colors"
              onClick={() => onRemoveTag(index)}
            >
              ×
            </button>
          </span>
        ))}
        <span className="text-gray-500 text-sm italic ml-2">
          {tags.length} / {maxTags}
        </span>
      </div>
    </div>
  );
};
import React from 'react';
import { Navbar } from '../layout/Navbar';
import { Footer } from '../layout/Footer';
import { BackgroundEffects } from '../layout/BackgroundEffects';
import { StatsCard } from '../home/StatsCard';
import { MethodStatsCard } from '../home/MethodStatsCard';
import { ViewAllCard } from '../home/ViewAllCard';

export const HomePage = () => {
  return (
    <div className="min-h-screen bg-gradient-to-br from-white via-white to-[#E4EFE7] text-[#1a1a1a] relative">
      <BackgroundEffects />
      <Navbar />

      <section className="pt-20 min-h-[84vh] flex items-center relative z-20">
        <div className="max-w-[1400px] mx-auto px-8 w-full">
          <div className="text-center max-w-6xl mx-auto py-8">
            <div className="grid grid-cols-2 gap-8 mt-12 max-w-5xl mx-auto">
              <StatsCard number="78" label="Total Endpoints" />
              <StatsCard number="23" label="Total Bookmarks" />
              <MethodStatsCard />
              <ViewAllCard />
            </div>
          </div>
        </div>
      </section>

      <Footer />
    </div>
  );
};
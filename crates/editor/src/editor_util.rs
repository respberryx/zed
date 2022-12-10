use std::{iter::Peekable, ops::Range};

use language::{Point, Selection};

use crate::display_map::DisplaySnapshot;

type RowIndex = u32;

pub fn end_row_for(selection: &Selection<Point>, display_map: &DisplaySnapshot) -> RowIndex {
    if selection.end.column > 0 || selection.is_empty() {
        display_map.next_line_boundary(selection.end).0.row + 1
    } else {
        selection.end.row
    }
}

pub struct ContiguousRowRanges<'snapshot, I: Iterator> {
    selections: Peekable<I>,
    display_map: &'snapshot DisplaySnapshot,
}

pub struct MergedOverlappingSelections<I: Iterator> {
    selections: Peekable<I>,
}

pub trait IteratorExtension {
    fn by_contiguous_rows(self, display_map: &DisplaySnapshot) -> ContiguousRowRanges<Self>
    where
        Self: Sized + Iterator<Item = Selection<Point>>,
    {
        ContiguousRowRanges {
            selections: self.peekable(),
            display_map,
        }
    }

    fn overlapping_selections_merged<T>(self) -> MergedOverlappingSelections<Self>
    where
        Self: Sized + Iterator<Item = Selection<T>>,
    {
        MergedOverlappingSelections {
            selections: self.peekable(),
        }
    }
}

impl<I> IteratorExtension for I where I: Iterator {}

impl<'snapshot, I: Iterator<Item = Selection<Point>>> Iterator
    for ContiguousRowRanges<'snapshot, I>
{
    type Item = (Range<u32>, Vec<Selection<Point>>);

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.selections.next();
        let mut selections = Vec::new();

        if let Some(selection) = next {
            selections.push(selection.clone());
            let start_row = selection.start.row;

            let mut end_row = end_row_for(&selection, self.display_map);

            while let Some(next_selection) = self.selections.peek() {
                if next_selection.start.row <= end_row {
                    end_row = end_row_for(next_selection, self.display_map);
                    selections.push(self.selections.next().unwrap().clone());
                } else {
                    break;
                }
            }
            Some((start_row..end_row, selections))
        } else {
            None
        }
    }
}

impl<T: Ord + Copy, I: Iterator<Item = Selection<T>>> Iterator for MergedOverlappingSelections<I> {
    type Item = Selection<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let selection = self.selections.next();

        selection.map(|mut selection| {
            while let Some(next_selection) = self.selections.peek() {
                if selection.end >= next_selection.start {
                    if next_selection.start < selection.start {
                        selection.start = next_selection.start;
                    }
                    if next_selection.end > selection.end {
                        selection.end = next_selection.end;
                    }

                    self.selections.next().unwrap();
                } else {
                    break;
                }
            }

            selection
        })
    }
}
